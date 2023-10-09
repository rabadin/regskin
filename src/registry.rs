use lazy_static::lazy_static;
use reqwest;
use reqwest::blocking::Client as BlockingClient;
use reqwest::blocking::Response as BlockingResponse;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

use regex::Regex;
use std::time::Duration;

use crate::tree::Tree;
use crate::vars;

fn get_client() -> Client {
    Client::builder()
        .danger_accept_invalid_certs(*vars::REGSKIN_IGNORE_INVALID_CERT)
        .gzip(true)
        .timeout(Duration::from_secs(300))
        .build()
        .unwrap()
}

fn get_sync_client() -> BlockingClient {
    reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(*vars::REGSKIN_IGNORE_INVALID_CERT)
        .gzip(true)
        .timeout(Duration::from_secs(300))
        .build()
        .unwrap()
}

#[derive(Deserialize, Debug, Clone)]
pub struct Token {
    pub token: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Catalog {
    pub repositories: Vec<String>,

    #[serde(skip)]
    pub tree: Tree,
}

impl Default for Catalog {
    fn default() -> Catalog {
        Catalog {
            repositories: vec![],
            tree: Tree {
                ..Default::default()
            },
        }
    }
}

impl Catalog {
    fn get_url() -> String {
        return format!(
            "{}/v2/_catalog?n={}",
            *vars::REGSKIN_REGISTRY_URL,
            *vars::REGSKIN_CATALOG_LIMIT
        );
    }

    fn get_token_url(scope: &str, service: &str) -> String {
        return format!(
            "{}/v2/token?service={}&scope={}",
            *vars::REGSKIN_REGISTRY_URL,
            scope,
            service
        );
    }

    pub async fn get_auth_header_if_401(
        response: &Response,
    ) -> Result<String, Box<dyn std::error::Error>> {
        lazy_static! {
            static ref REGEX: Regex =
                Regex::new(r#"(?i)service="([^"]+)",\s*scope="([^"]+)""#).unwrap();
        }
        if response.status() == StatusCode::UNAUTHORIZED {
            let auth = response.headers()["www-authenticate"].to_str()?;
            if let Some(captures) = REGEX.captures(auth) {
                let service = captures.get(1).unwrap().as_str();
                let scope = captures.get(2).unwrap().as_str();
                let token_url = &Catalog::get_token_url(service, scope);
                let token: Token = get_client().get(token_url).send().await?.json().await?;
                return Ok(format!("Bearer {}", token.token));
            }
        }
        return Ok("".to_string());
    }

    pub fn get_auth_header_if_401_sync(
        response: &BlockingResponse,
    ) -> Result<String, Box<dyn std::error::Error>> {
        lazy_static! {
            static ref REGEX: Regex =
                Regex::new(r#"(?i)service="([^"]+)",\s*scope="([^"]+)""#).unwrap();
        }
        if response.status() == StatusCode::UNAUTHORIZED {
            let auth = response.headers()["www-authenticate"].to_str()?;
            if let Some(captures) = REGEX.captures(auth) {
                let service = captures.get(1).unwrap().as_str();
                let scope = captures.get(2).unwrap().as_str();
                let token_url = &Catalog::get_token_url(service, scope);
                let token: Token = get_sync_client().get(token_url).send()?.json()?;
                return Ok(format!("Bearer {}", token.token));
            }
        }
        return Ok("".to_string());
    }

    pub fn get_sync() -> Result<Catalog, Box<dyn std::error::Error>> {
        let mut catalog: Catalog;
        let mut response = get_sync_client().get(&Catalog::get_url()).send()?;
        let auth = &Catalog::get_auth_header_if_401_sync(&response)?;
        if auth != "" {
            response = get_sync_client()
                .get(&Catalog::get_url())
                .header("Authorization", auth)
                .send()?;
        }
        catalog = response.json()?;
        catalog.update_tree();
        Ok(catalog)
    }

    fn update_tree(&mut self) {
        let mut structure = Tree::new();
        for repo in &self.repositories {
            structure.add_path(repo);
        }
        self.tree = structure;
    }

    pub async fn get_tags(&self, path: &str) -> Result<Tags, Box<dyn std::error::Error>> {
        let mut repo = path.to_string();
        repo.pop();
        if !self.repositories.contains(&repo) {
            return Ok(Tags::new());
        }
        let client = get_client();
        let url = format!("{}/v2/{}tags/list", *vars::REGSKIN_REGISTRY_URL, path);
        let mut response = client
            .get(&url)
            .header(
                "Accept",
                "application/vnd.docker.distribution.manifest.v2+json",
            )
            .send()
            .await?;
        let auth = &Catalog::get_auth_header_if_401(&response).await?;
        if auth != "" {
            response = client
                .get(&url)
                .header("Authorization", auth)
                .send()
                .await?;
        }
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(Tags::new());
        } else if response.status().is_success() {
            let mut tags: Tags = response.json().await?;
            tags.tags.sort();
            tags.tags.reverse();
            return Ok(tags);
        }
        Ok(Tags::new())
    }

    pub async fn get_image_data(
        &self,
        path: &str,
        tag: &str,
    ) -> Result<ImageV1, Box<dyn std::error::Error>> {
        let url = format!(
            "{}/v2/{}/manifests/{}",
            *vars::REGSKIN_REGISTRY_URL,
            path,
            tag
        );
        let client = get_client();
        let mut response = client.get(&url).send().await?;
        let auth = &Catalog::get_auth_header_if_401(&response).await?;
        if auth != "" {
            response = client
                .get(&url)
                .header("Authorization", auth)
                .send()
                .await?;
        }
        let mut image: ImageV1 = response.json().await?;
        let mut details: ImageV1Details =
            serde_json::from_str(image.history[0].get("v1Compatibility").unwrap())?;
        details.update_config();
        details.tag = tag.to_string();
        details.path = path.to_string();
        image.details = details;
        Ok(image)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Tags {
    pub name: String,
    pub tags: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Dir {
    pub tags: Vec<String>,
    pub dirs: Vec<String>,
}

impl Tags {
    pub fn new() -> Tags {
        Tags {
            name: "".to_string(),
            tags: vec![],
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ImageV1 {
    pub name: String,
    pub history: Vec<HashMap<String, String>>,
    #[serde(skip)]
    pub details: ImageV1Details,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ImageV1Details {
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub architecture: String,
    pub config: Value,
    pub created: String,
    #[serde(default)]
    pub docker_version: String,
    pub os: String,
    #[serde(skip)]
    pub config_parsed: Config,
}

impl Default for ImageV1Details {
    fn default() -> ImageV1Details {
        ImageV1Details {
            config_parsed: Config {
                ..Default::default()
            },
            tag: "".to_string(),
            path: "".to_string(),
            architecture: "".to_string(),
            config: json!(""),
            created: "".to_string(),
            docker_version: "".to_string(),
            os: "".to_string(),
        }
    }
}

impl ImageV1Details {
    fn update_config(&mut self) {
        let config = if self.config["Labels"] != json!(null) {
            serde_json::from_value(self.config.clone()).unwrap()
        } else {
            Config {
                ..Default::default()
            }
        };
        self.config_parsed = config;
    }
}
#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    #[serde(alias = "Labels")]
    pub labels: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            labels: BTreeMap::new(),
        }
    }
}
