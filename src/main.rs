#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use actix_files as fs;
use actix_web::http::header;
use actix_web::middleware::{DefaultHeaders, Logger};
use actix_web::{guard, middleware, web, App, HttpResponse, HttpServer, Responder, Result};
use actix_web_prom::PrometheusMetricsBuilder;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::RwLock;

use actix_web::error::{ErrorInternalServerError, ErrorNotFound};
use askama::Template;
use std::thread;
use std::time::Duration;

use self::registry::{Catalog, ImageV1};

mod registry;
mod tree;
mod vars;

async fn healthz() -> HttpResponse {
    HttpResponse::Ok().body("Ok")
}

async fn favicon() -> HttpResponse {
    HttpResponse::MovedPermanently()
        .insert_header((header::LOCATION, "/static/favicon.ico"))
        .finish()
}

#[derive(Template)]
#[template(path = "directory.html")]
struct DirectoryTemplate {
    tags: Vec<String>,
    dirs: Vec<String>,
    path: String,
    path_stripped: String,
    registry: String,
}

async fn directory(data: web::Data<State>, path: web::Path<String>) -> Result<HttpResponse> {
    let catalog = data.catalog.read().unwrap();
    let mut full_path = path.clone();
    let mut full_path_stripped = path.clone();
    if full_path != "" {
        if full_path_stripped.ends_with("/") {
            full_path_stripped.pop();
        } else {
            full_path = full_path + "/"
        }
    }
    let node = catalog.tree.get_path(&full_path);
    match node {
        None => Err(ErrorNotFound("Not found")),
        Some(ref n) => {
            let tags = catalog
                .get_tags(&full_path)
                .await
                .map_err(ErrorInternalServerError)?;
            let template = DirectoryTemplate {
                dirs: n.sorted_childrens(),
                path: full_path,
                path_stripped: full_path_stripped,
                tags: tags.tags,
                registry: vars::REGSKIN_REGISTRY_HOST.to_string(),
            }
            .render()
            .unwrap();
            Ok(HttpResponse::Ok().content_type("text/html").body(template))
        }
    }
}

async fn directory_json(data: web::Data<State>, path: web::Path<String>) -> Result<impl Responder> {
    let catalog = data.catalog.read().unwrap();
    let mut full_path = path.clone();
    let mut full_path_stripped = path.clone();
    if full_path != "" {
        if full_path_stripped.ends_with("/") {
            full_path_stripped.pop();
        } else {
            full_path = full_path + "/"
        }
    }
    let node = catalog.tree.get_path(&full_path);
    match node {
        None => Err(ErrorNotFound("Not found")),
        Some(ref n) => {
            let tags = catalog
                .get_tags(&full_path)
                .await
                .map_err(ErrorInternalServerError)?;
            let dir = registry::Dir {
                tags: tags.tags,
                dirs: n.sorted_childrens(),
            };
            Ok(web::Json(dir))
        }
    }
}

#[derive(Template)]
#[template(path = "tag.html")]
struct TagTemplate {
    path: String,
    registry: String,
    tag: String,
    image: ImageV1,
}

async fn tag(data: web::Data<State>, path: web::Path<(String, String)>) -> Result<HttpResponse> {
    let catalog = data.catalog.read().unwrap();
    let full_path = path.clone().0;
    let tag = path.1.clone();
    let image = catalog
        .get_image_data(&full_path, &tag)
        .await
        .map_err(ErrorInternalServerError)?;
    let template = TagTemplate {
        path: full_path,
        tag,
        image,
        registry: vars::REGSKIN_REGISTRY_HOST.to_string(),
    }
    .render()
    .unwrap();
    Ok(HttpResponse::Ok().content_type("text/html").body(template))
}

async fn tag_json(
    data: web::Data<State>,
    path: web::Path<(String, String)>,
) -> Result<impl Responder> {
    let catalog = data.catalog.read().unwrap();
    let full_path = path.clone().0;
    let tag = path.1.clone();
    let image = catalog
        .get_image_data(&full_path, &tag)
        .await
        .map_err(ErrorInternalServerError)?;
    Ok(web::Json(image.details))
}

#[derive(Clone)]
pub struct State {
    catalog: Arc<RwLock<Catalog>>,
}

fn update_catalog(guard: &Arc<RwLock<Catalog>>) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Updating registry catalog");
    let new_catalog = registry::Catalog::get_sync()?;
    let mut catalog = guard.write().unwrap();
    let _ = std::mem::replace(&mut *catalog, new_catalog);
    Ok(())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env("REGSKIN_LOG_LEVEL").init();
    info!("Starting server {}", vars::SERVER_BANNER.to_string());

    let catalog = Catalog {
        ..Default::default()
    };
    let guard_catalog = Arc::new(RwLock::new(catalog));
    let guard = guard_catalog.clone();

    thread::spawn(move || loop {
        info!("Updating catalog...");
        let _ = update_catalog(&guard)
            .map_err(|e| error!("{}", e))
            .map(|_| info!("Catalog fetched"));
        thread::sleep(Duration::from_millis(1000 * 60 * 10));
    });

    loop {
        thread::sleep(Duration::from_millis(2000));
        if guard_catalog.clone().read().unwrap().repositories.len() != 0 {
            break;
        }
    }

    let state = State {
        catalog: guard_catalog.clone(),
    };
    let prometheus = PrometheusMetricsBuilder::new("regskin")
        .endpoint("/metrics")
        .build()
        .unwrap();
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .wrap(Logger::default())
            .wrap(middleware::NormalizePath::trim())
            .wrap(DefaultHeaders::new().add((header::SERVER, vars::SERVER_BANNER.to_string())))
            .service(fs::Files::new("/static/", "static").show_files_listing())
            .service(web::resource("/favicon.ico").route(web::get().to(favicon)))
            .service(web::resource("/healthz").route(web::get().to(healthz)))
            .service(
                web::resource("/{path:[^:]*}")
                    .route(
                        web::route()
                            .guard(guard::Get())
                            .guard(guard::Header("content-type", "application/json"))
                            .to(directory_json),
                    )
                    .route(web::get().to(directory)),
            )
            .service(
                web::resource("/{path:[^:]*}:{tag:.*}")
                    .route(
                        web::route()
                            .guard(guard::Get())
                            .guard(guard::Header("content-type", "application/json"))
                            .to(tag_json),
                    )
                    .route(web::get().to(tag)),
            )
            .service(web::resource("/{path:[^:]*}:{tag:.*}").route(web::head().to(tag)))
            .wrap(prometheus.clone())
    })
    .backlog(2048)
    .bind(SocketAddr::from((
        *vars::REGSKIN_LISTEN,
        *vars::REGSKIN_PORT,
    )))?
    .run()
    .await
}
