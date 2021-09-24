use actix_web::{web, get, App, HttpServer, HttpResponse};
use std::clone::Clone;
use serde::{Serialize};

mod storage {
    use s3::creds::Credentials;
    use s3::region::Region;
    use s3::bucket::Bucket;
    use std::env;

    #[derive(Clone)]
    pub struct Storage {
        bucket: Bucket,
    }

    impl Storage {
        pub fn new() -> Storage {
            let role = env::var("AWS_ROLE").unwrap_or(String::from("default"));
            let bucket = env::var("IMAGE_BUCKET").unwrap_or_else(|_err| {
                panic!("Must provide s3 bucket through env var IMAGE_BUCKET")
            });
            let region = env::var("AWS_REGION").unwrap_or(String::from("us-east-1"));

            let credentials = Credentials::new(None,None,None,None,Some(&role)).unwrap_or_else(|_err| {
                panic!("Invalid credentials role: {}", role)
            });
            let region: Region = region.parse().unwrap_or(Region::UsEast1);
            let bucket = Bucket::new(&bucket, region, credentials).unwrap_or_else(|err| {
                panic!("Failed to create bucket: {}", err)
            });

            Storage {
                bucket
            }
        }

        pub async fn list(&self) -> Vec<String> {
            let image_prefix = "images/";
            let results = self.bucket.list(image_prefix.to_string(), Some("/".to_string())).await
                .unwrap_or_else(|error| {
                    panic!("failed to list bucket {:?}", error)
                });

            results
                .into_iter()
                .flat_map(|result| result.contents)
                .map(|file| file.key.strip_prefix(image_prefix).unwrap().to_string())
                .filter(|file| file != "")
                .collect()
        }
    }
}

#[derive(Serialize)]
struct ListResponse {
    files: Vec<String>
}

#[get("/_list")]
async fn handle_image_upload(context: web::Data<ServerContext>) -> HttpResponse {
    let files = context.storage.list().await;
    HttpResponse::Ok().json(ListResponse { files })
}

use storage::Storage;

#[derive(Clone)]
struct ServerContext {
    storage: Storage
}

impl ServerContext {
    fn new() -> ServerContext {
        ServerContext {
            storage: Storage::new(),
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let context = ServerContext::new();

    HttpServer::new(move || {
        App::new()
            .data(context.clone())
            .service(handle_image_upload)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
