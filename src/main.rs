use actix_web::{get, App, HttpServer, HttpResponse};
use std::env;
use serde::{Serialize};
use s3::creds::Credentials;
use s3::region::Region;
use s3::bucket::Bucket;

#[get("/")]
async fn hello() -> &'static str {
    "Hello world, rust!\r\n"
}

#[derive(Serialize)]
struct ListResponse {
    files: Vec<String>
}

#[get("/list")]
async fn handle_image_upload() -> HttpResponse {
    let role = env::var("AWS_ROLE").unwrap_or(String::from("default"));
    let bucket = env::var("IMAGE_BUCKET").unwrap_or_else(|_err| {
        panic!("Must provide s3 bucket through env var IMAGE_BUCKET")
    });
    let region = env::var("AWS_REGION").unwrap_or(String::from("us-east-1"));

    let credentials = Credentials::new(None,None,None,None,Some(&role));
    let region: Region = region.parse().unwrap();
    let bucket = Bucket::new(&bucket, region, credentials.unwrap()).unwrap();

    let results = bucket.list("".to_string(), Some("/".to_string())).await
        .unwrap_or_else(|error| {
            panic!("failed to list bucket {:?}", error)
        });

    let files: Vec<String> = results
        .into_iter()
        .flat_map(|result| result.contents)
        .map(|file| file.key)
        .collect();

    HttpResponse::Ok().json(ListResponse { files })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(hello)
            .service(handle_image_upload)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
