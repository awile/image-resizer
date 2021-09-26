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

        pub async fn create(&self, content: &[u8], image_name: &str, width: Option<u32>, height: Option<u32>) -> u16 {
            let mut folder = IMAGE_FOLDER;
            let mut name = format!("{}", image_name);
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

#[derive(Clone)]
struct ImageService {
    storage: storage::Storage
}

impl ImageService {
    pub fn new() -> ImageService {
        let storage = storage::Storage::new();
        ImageService {
            storage
        }
    }

    fn get_image_output_format(file_name: &str) -> Option<ImageOutputFormat> {
        let name_parts: Vec<&str> = file_name.split(".").collect();
        if name_parts.len() == 2 {
            match name_parts[1] {
                "jpeg" => Some(ImageOutputFormat::Jpeg(75)),
                "jpg" => Some(ImageOutputFormat::Jpeg(75)),
                "png" => Some(ImageOutputFormat::Png),
                "ico" => Some(ImageOutputFormat::Ico),
                "gif" => Some(ImageOutputFormat::Gif),
                _ => None
            }
        } else {
            None
        }
    }

    fn get_image_format(file_name: &str) -> Option<ImageFormat> {
        let name_parts: Vec<&str> = file_name.split(".").collect();
        if name_parts.len() == 2 {
            match name_parts[1] {
                "jpeg" => Some(ImageFormat::Jpeg),
                "jpg" => Some(ImageFormat::Jpeg),
                "png" => Some(ImageFormat::Png),
                "ico" => Some(ImageFormat::Ico),
                "gif" => Some(ImageFormat::Gif),
                _ => None
            }
        } else {
            None
        }
    }

    fn get_content_header(format: &ImageFormat) -> &'static str {
        match format {
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png",
            ImageFormat::Ico => "image/ico",
            ImageFormat::Gif => "image/gif",
            _ => "image/jpeg"
        }
    }

    async fn get_image(&self, image_name: &str, width: Option<u32>, height: Option<u32>) -> Option<(Vec<u8>, &'static str)> {
        let image_format = ImageService::get_image_format(&image_name).unwrap();
        let image_format_header = ImageService::get_content_header(&image_format);

        let image = self.storage.get(image_name.to_string(), width, height).await;
        if image.is_some() {
            Some((image.unwrap(), image_format_header))
        } else if width.is_none() && height.is_none() {
            None
        } else {
            let original_image = self.storage.get(image_name.to_string(), None, None).await;
            if original_image.is_none() {
                None
            } else {
                let img = image::load_from_memory_with_format(&original_image.unwrap(), image_format).unwrap_or_else(|err| {
                    panic!("failed to load image {}", err)
                });

                let image_output_format = ImageService::get_image_output_format(&image_name);
                if image_output_format.is_none() {
                    return None
                }
                let mut w: Cursor<Vec<u8>> = Cursor::new(Vec::new());
                let resized_image = img.resize_exact(width.unwrap(), height.unwrap(), FilterType::Nearest);
                resized_image.write_to(&mut w, image_output_format.unwrap()).unwrap();
                let image_bytes = w.into_inner();
                self.storage.create(&image_bytes, &image_name, width, height).await;
                Some((image_bytes, image_format_header))
            }
        }
    }

    async fn list(&self) -> Vec<String> {
        self.storage.list().await
    }

    async fn create(&self, content: &[u8], content_type: &str) -> (String, u16) {
        let mime_type: Vec<&str> = content_type.split("/").collect();
        let image_name = format!("{}.{}", Uuid::new_v4(), mime_type.last().unwrap());
        let code = self.storage.create(content, &image_name, None, None).await;
        (image_name, code)
    }
}

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
    let content_type = content_type_header.to_str().unwrap();
    let (filename, code) = context.image_service.create(&body, &content_type).await;
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
