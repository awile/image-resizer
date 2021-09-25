use actix_web::{web, error, App, HttpServer, HttpRequest, HttpResponse, Error};
use std::clone::Clone;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use image::{ ImageFormat, ImageOutputFormat };
use image::imageops::FilterType;
use std::io::Cursor;

mod storage {
    use s3::creds::Credentials;
    use s3::region::Region;
    use s3::bucket::Bucket;
    use std::env;
    use std::str;

    const IMAGE_FOLDER: &str = "images/";
    const CACHE_FOLDER: &str = "cache/";

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

        pub async fn create(&self, content: &[u8], image_name: String, width: Option<u32>, height: Option<u32>) -> u16 {
            let mut folder = IMAGE_FOLDER;
            let mut name = image_name;
            if width.is_some() || height.is_some() {
                folder = CACHE_FOLDER;
                name = format!("{}_{}_{}", name, width.unwrap_or(0), height.unwrap_or(0));
            }
            let (_, code) = self.bucket.put_object(format!("{}{}", folder, name), &content).await.unwrap_or_else(|err| {
                panic!("failed to upload image to bucket: {:?}", err)
            });
            code
        }

        pub async fn get(&self, image_name: String, width: Option<u32>, height: Option<u32>) -> Option<Vec<u8>> {
            let mut folder = IMAGE_FOLDER;
            let mut name = image_name;
            if width.is_some() || height.is_some() {
                folder = CACHE_FOLDER;
                name = format!("{}_{:?}_{:?}", name, width.unwrap_or(0), height.unwrap_or(0));
            }
            let (data, code) = self.bucket.get_object(format!("{}{}", folder, name)).await.unwrap();
            match code {
                200 => Some(data),
                _ => None
            }
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

async fn handle_image_upload(req: HttpRequest, body: web::Bytes, context: web::Data<ServerContext>) -> Result<HttpResponse, Error> {
    let content_type_header = req.headers().get("Content-Type").unwrap_or_else(|| {
        panic!("No Content Type")
    });
    let mime_type: Vec<&str> = content_type_header.to_str().unwrap().split("/").collect();
    let filename = format!("{}.{}", Uuid::new_v4(), mime_type.last().unwrap());
    let code = context.storage.create(&body, filename.to_string(), None, None).await;
    let resp = UploadResponse { id: filename };
    match code {
        200 => Ok(HttpResponse::Ok().json(resp)),
        _ => Err(error::ErrorBadRequest("failed to upload image")),
    }

}

#[derive(Deserialize)]
struct GetImageParams {
    w: Option<u32>,
    h: Option<u32>,
}

async fn handle_image_get(image_id: web::Path<String>, params: web::Query<GetImageParams>, context: web::Data<ServerContext>) -> Result<HttpResponse, Error> {
    let image = context.storage.get(image_id.to_string(), params.w, params.h).await;
    if image.is_some() {
        Ok(HttpResponse::Ok()
                .content_type("image/jpeg")
                .body(image.unwrap()))
    } else if params.w.is_none() && params.h.is_none() {
        Err(error::ErrorNotFound("no image found"))
    } else {
        let original_image = context.storage.get(image_id.to_string(), None, None).await;
        if original_image.is_none() {
            Err(error::ErrorNotFound("no image found"))
        } else {
            let img = image::load_from_memory_with_format(&original_image.unwrap(), ImageFormat::Jpeg).unwrap_or_else(|err| {
                panic!("failed to load image {}", err)
            });
            let mut w: Cursor<Vec<u8>> = Cursor::new(Vec::new());
            let resized_image = img.resize_exact(params.w.unwrap(), params.h.unwrap(), FilterType::Nearest);
            resized_image.write_to(&mut w, ImageOutputFormat::Jpeg(75)).unwrap();
            let image_bytes = w.into_inner();
            context.storage.create(&image_bytes, image_id.to_string(), params.w, params.h).await;
            Ok(HttpResponse::Ok()
                .content_type("image/jpeg")
                .body(image_bytes))
        }
    }
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
            .route("/{id}", web::get().to(handle_image_get))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
