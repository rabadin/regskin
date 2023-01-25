use reqwest;
use reqwest::blocking::Client as BlockingClient;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use crate::tree::Tree;
use crate::vars;

fn get_client() -> Client {
    Client::builder()
        .gzip(true)
        .timeout(Duration::from_secs(100))
        .build()
        .unwrap()
}

fn get_sync_client() -> BlockingClient {
    reqwest::blocking::Client::builder()
        .gzip(true)
        .timeout(Duration::from_secs(100))
        .build()
        .unwrap()
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
    pub fn get_sync() -> Result<Catalog, Box<dyn std::error::Error>> {
        let mut catalog: Catalog = get_sync_client().get(&Catalog::get_url()).send()?.json()?;
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
        let url = format!("{}/v2/{}/tags/list", *vars::REGSKIN_REGISTRY_URL, path);
        let response = client
            .get(&url)
            .header(
                "Accept",
                "application/vnd.docker.distribution.manifest.v2+json",
            )
            .send()
            .await?;
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
        let mut image: ImageV1 = client.get(&url).send().await?.json().await?;
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
