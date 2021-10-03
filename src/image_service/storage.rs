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
        let results = self.bucket.list(IMAGE_FOLDER.to_string(), Some("/".to_string())).await;
        match results {
            Err(_) => Vec::new(),
            Ok(images) =>
                images
                    .into_iter()
                    .flat_map(|result| result.contents)
                    .filter_map(|file| match file.key.strip_prefix(IMAGE_FOLDER) {
                        Some("") => None,
                        Some(image_key) => Some(image_key.to_string()),
                        _ => None
                    })
                    .collect()
        }
    }

    pub async fn create(&self, content: &[u8], image_name: &str, width: Option<u32>, height: Option<u32>) -> Result<u16, &'static str> {
        let mut folder = IMAGE_FOLDER;
        let mut name = format!("{}", image_name);
        if width.is_some() || height.is_some() {
            folder = CACHE_FOLDER;
            name = format!("{}_{}_{}", name, width.unwrap_or(0), height.unwrap_or(0));
        }
        let create_resp = self.bucket.put_object(format!("{}{}", folder, name), &content).await;
        match create_resp {
            Ok((_, 200)) => Ok(200),
            _ => Err("failed to upload image")
        }
    }

    pub async fn get(&self, image_name: String, width: Option<u32>, height: Option<u32>) -> Option<Vec<u8>> {
        let mut folder = IMAGE_FOLDER;
        let mut name = image_name;
        if width.is_some() || height.is_some() {
            folder = CACHE_FOLDER;
            name = format!("{}_{:?}_{:?}", name, width.unwrap_or(0), height.unwrap_or(0));
        }
        let bucket_resp = self.bucket.get_object(format!("{}{}", folder, name)).await;
        match bucket_resp {
            Ok((data, 200)) => Some(data),
            _ => None
        }
    }
}
