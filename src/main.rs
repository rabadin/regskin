#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use actix_files as fs;
use actix_web::http::header;
use actix_web::middleware::{DefaultHeaders, Logger};
use actix_web::{middleware, web, App, HttpResponse, HttpServer, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::RwLock;

use actix_web::error::{ErrorInternalServerError, ErrorNotFound};
use askama::Template;
use clokwerk::{Scheduler, TimeUnits};
use std::time::Duration;

use self::registry::{Catalog, ImageV1};

mod registry;
mod tree;
mod vars;

fn healthz() -> Result<String> {
    Ok("Ok".to_string())
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

fn directory(data: web::Data<State>, path: web::Path<(String,)>) -> Result<HttpResponse> {
    let catalog = data.catalog.read().unwrap();
    let full_path = path.0.clone();
    let mut full_path_stripped = path.0.clone();
    full_path_stripped.pop();
    let node = catalog.tree.get_path(&full_path);
    match node {
        None => Err(ErrorNotFound("Not found")),
        Some(ref n) => {
            let tags = catalog
                .get_tags(&full_path)
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

#[derive(Template)]
#[template(path = "tag.html")]
struct TagTemplate {
    path: String,
    registry: String,
    tag: String,
    image: ImageV1,
}

fn tag(data: web::Data<State>, path: web::Path<(String, String)>) -> Result<HttpResponse> {
    let catalog = data.catalog.read().unwrap();
    let full_path = path.0.clone();
    let tag = path.1.clone();
    let image = catalog
        .get_image_data(&full_path, &tag)
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

#[derive(Clone)]
pub struct State {
    catalog: Arc<RwLock<Catalog>>,
}

fn update_catalog(guard: &Arc<RwLock<Catalog>>) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Updating registry catalog");
    let new_catalog = registry::Catalog::get()?;
    let mut catalog = guard.write().unwrap();
    std::mem::replace(&mut *catalog, new_catalog);
    Ok(())
}

fn main() {
    env_logger::Builder::from_env("REGSKIN_LOG_LEVEL").init();
    info!("Starting server {}", vars::SERVER_BANNER.to_string());

    let catalog = Catalog {
        ..Default::default()
    };
    let guard_catalog = Arc::new(RwLock::new(catalog));
    let guard = guard_catalog.clone();

    info!("Getting initial catalog...");
    let _ = update_catalog(&guard)
        .map_err(|e| error!("{}", e))
        .map(|_| info!("Catalog fetched"));

    let mut scheduler = Scheduler::new();
    scheduler.every(10.minutes()).run(move || {
        let _ = update_catalog(&guard).map_err(|e| error!("{}", e));
    });
    let _thread = scheduler.watch_thread(Duration::from_millis(100));

    let state = State {
        catalog: guard_catalog.clone(),
    };
    HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .wrap(Logger::default())
            .wrap(middleware::NormalizePath)
            .wrap(DefaultHeaders::new().header(header::SERVER, vars::SERVER_BANNER.to_string()))
            .service(fs::Files::new("/static/", "static").show_files_listing())
            .route("/{path:[^:]*}", web::get().to(directory))
            .route("/{path:[^:]*}", web::head().to(directory))
            .route("/{path:[^:]*}:{tag:.*}", web::get().to(tag))
            .route("/{path:[^:]*}:{tag:.*}", web::head().to(tag))
            .route("/healthz/", web::get().to(healthz))
    })
    .backlog(2048)
    .bind(SocketAddr::from((
        *vars::REGSKIN_LISTEN,
        *vars::REGSKIN_PORT,
    )))
    .unwrap()
    .run()
    .unwrap();
    info!("Stopping server");
}
