mod image_service;

use actix_web::{web, error, App, HttpServer, HttpRequest, HttpResponse, Error};
use std::clone::Clone;
use serde::{Deserialize, Serialize};
use image_service::ImageService;

#[derive(Serialize)]
struct ListResponse {
    files: Vec<String>
}

async fn handle_image_list(context: web::Data<ServerContext>) -> Result<HttpResponse, Error> {
    let files = context.image_service.list().await;
    Ok(HttpResponse::Ok().json(ListResponse { files }))
}

#[derive(Serialize)]
struct UploadResponse {
    id: String
}

async fn handle_image_upload(req: HttpRequest, body: web::Bytes, context: web::Data<ServerContext>) -> Result<HttpResponse, Error> {
    let content_type_header = req.headers().get("Content-Type").unwrap_or_else(|| {
        panic!("No Content Type")
    });
    let content_type = content_type_header.to_str().unwrap_or("");
    let create_resp = context.image_service.create(&body, &content_type).await;
    match create_resp {
        Ok(image_name) => Ok(HttpResponse::Ok().json(UploadResponse{ id: image_name})),
        _ => Err(error::ErrorBadRequest("failed to upload image")),
    }
}

#[derive(Deserialize)]
struct GetImageParams {
    w: Option<u32>,
    h: Option<u32>,
}

async fn handle_image_get(image_name: web::Path<String>, params: web::Query<GetImageParams>, context: web::Data<ServerContext>) -> Result<HttpResponse, Error> {
    let image = context.image_service.get_image(&image_name, params.w, params.h).await;
    match image {
        Some((image_data, content_type_header)) =>
            Ok(HttpResponse::Ok()
                .content_type(content_type_header)
                .body(image_data)),
        None => Err(error::ErrorNotFound("no image found")),
    }
}

#[derive(Clone)]
struct ServerContext {
    image_service: ImageService
}

impl ServerContext {
    fn new() -> ServerContext {
        ServerContext {
            image_service: ImageService::new(),
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let context = ServerContext::new();

    HttpServer::new(move || {
        App::new()
            .data(context.clone())
            .route("/_list", web::get().to(handle_image_list))
            .route("/upload", web::post().to(handle_image_upload))
            .route("/{id}", web::get().to(handle_image_get))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
