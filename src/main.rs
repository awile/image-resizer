use actix_web::{web, App, HttpServer, HttpResponse, Error};
use std::clone::Clone;
use serde::{Serialize};

mod storage {
    use s3::creds::Credentials;
    use s3::region::Region;
    use s3::bucket::Bucket;
    use std::env;
    use uuid::Uuid;

    const IMAGE_FOLDER: &str = "images/";

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
            let results = self.bucket.list(IMAGE_FOLDER.to_string(), Some("/".to_string())).await
                .unwrap_or_else(|error| {
                    panic!("failed to list bucket {:?}", error)
                });

            results
                .into_iter()
                .flat_map(|result| result.contents)
                .map(|file| file.key.strip_prefix(IMAGE_FOLDER).unwrap().to_string())
                .filter(|file| file != "")
                .collect()
        }

        pub async fn create(&self, content: &[u8]) -> String {
            let image_id = Uuid::new_v4();
            self.bucket.put_object(format!("{}/{}.jpg", IMAGE_FOLDER, image_id), &content).await.unwrap_or_else(|err| {
                panic!("failed to upload image to bucket: {:?}", err)
            });
            image_id.to_string()
        }
    }
}

#[derive(Serialize)]
struct ListResponse {
    files: Vec<String>
}

async fn handle_image_list(context: web::Data<ServerContext>) -> Result<HttpResponse, Error> {
    let files = context.storage.list().await;
    Ok(HttpResponse::Ok().json(ListResponse { files }))
}

#[derive(Serialize)]
struct UploadResponse {
    id: String
}

async fn handle_image_upload(bytes: web::Bytes, context: web::Data<ServerContext>) -> Result<HttpResponse, Error> {
    let id = context.storage.create(&bytes).await;
    let resp = UploadResponse { id };
    Ok(HttpResponse::Ok().json(resp))
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
            .route("/_list", web::get().to(handle_image_list))
            .route("/upload", web::post().to(handle_image_upload))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
