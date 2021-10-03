
mod storage;

use uuid::Uuid;
use image::{ ImageFormat, ImageOutputFormat };
use image::imageops::FilterType;
use std::io::Cursor;
use storage::Storage;
use std::clone::Clone;
use image::GenericImageView;

#[derive(Clone)]
pub struct ImageService {
    storage: Storage
}

impl ImageService {
    pub fn new() -> ImageService {
        let storage = Storage::new();
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

    pub async fn list(&self) -> Vec<String> {
        self.storage.list().await
    }

    pub async fn create(&self, content: &[u8], content_type: &str) -> Result<String, &'static str> {
        let mime_type: &str = content_type.split("/").last().unwrap_or("");
        let image_name = format!("{}.{}", Uuid::new_v4(), mime_type);
        let create_resp = self.storage.create(content, &image_name, None, None).await;
        match create_resp {
            Ok(_code) => Ok(image_name),
            Err(err) => Err(err)
        }
    }

    pub async fn get_image(&self, image_name: &str, width: Option<u32>, height: Option<u32>) -> Option<(Vec<u8>, &'static str)> {
        let image_format = ImageService::get_image_format(&image_name).unwrap_or(ImageFormat::Jpeg);
        let image_format_header = ImageService::get_content_header(&image_format);

        let image = self.storage.get(image_name.to_string(), width, height).await;
        match (image, width, height) {
            (Some(data), _, _) => Some((data, image_format_header)),
            (None, None, None) => None,
            _ => {
                let original_image = self.storage.get(image_name.to_string(), None, None).await?;
                let image_write_format = ImageService::get_image_output_format(&image_name)?;
                let resized_image = self.resize_image(&original_image, width, height, &image_format, image_write_format)?;
                let create_resp = self.storage.create(&resized_image, &image_name, width, height).await;
                if create_resp.is_err() {
                    println!("failed to cache image resize: {:?}", create_resp.err());
                }
                Some((resized_image, image_format_header))
            }
        }
    }

    fn resize_image(&self, image_data: &Vec<u8>, width: Option<u32>, height: Option<u32>, image_read_format: &ImageFormat, image_write_format: ImageOutputFormat) -> Option<Vec<u8>> {
        let img = image::load_from_memory_with_format(image_data, *image_read_format).unwrap_or_else(|err| {
            panic!("failed to load image {}", err)
        });
        let mut w: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        let (new_width, new_height) = self.get_resize_dimensions(width, height, img.dimensions());
        let resized_image = img.resize_exact(new_width, new_height, FilterType::Nearest);
        let write_resp = resized_image.write_to(&mut w, image_write_format);
        match write_resp {
            Ok(_) => Some(w.into_inner()),
            Err(_) => None
        }
    }

    fn get_resize_dimensions(&self, width: Option<u32>, height: Option<u32>, dimensions: (u32, u32)) -> (u32, u32) {
        match (width, height) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => {
                let new_height = (dimensions.1 as f64 / dimensions.0 as f64) * w as f64;
                (w, new_height as u32)
            },
            (None, Some(h)) => {
                let new_width = (dimensions.0 as f64 / dimensions.1 as f64) * h as f64;
                (new_width as u32, h)
            },
            _ => dimensions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_content_header() {
        let image_formats: [ImageFormat; 4] = [
            ImageFormat::Jpeg,
            ImageFormat::Png,
            ImageFormat::Ico,
            ImageFormat::Gif,
        ];
        let content_headers: [&'static str; 4] = [
            "image/jpeg",
            "image/png",
            "image/ico",
            "image/gif",
        ];
        for (index, img_format) in image_formats.iter().enumerate() {
            let expected_content_header = content_headers[index];
            let actual_content_header = ImageService::get_content_header(img_format);
            assert_eq!(expected_content_header, actual_content_header);
        }
    }

    #[test]
    fn test_get_image_format() {
        let files: [&'static str; 8] = [
            "16029914-329c-404a-afe9-5a5a321f6824.jpeg",
            "aaa22222-d2a9-4a3e-8a83-6aa7abe1a784.jpg",
            "cf2a1735-d2a9-4a3e-8a83-6aa7abe1a784.png",
            "ff44a321-d2a9-4a3e-8a83-6aa7abe1a784.ico",
            "d880aa7f-d2a9-4a3e-8a83-6aa7abe1a784.gif",
            "33229914-329c-404a-afe9-5a5a321f6824.mov",
            "f91dbfb9-59eb-4bd2-8347-6ff372be40e4",
            "f91dbfb9-59eb-4bd2-8347-6ff372be40e4.name.png",
        ];
        let format: [Option<ImageFormat>; 8] = [
            Some(ImageFormat::Jpeg),
            Some(ImageFormat::Jpeg),
            Some(ImageFormat::Png),
            Some(ImageFormat::Ico),
            Some(ImageFormat::Gif),
            None,
            None,
            None,
        ];
        for (index, file) in files.iter().enumerate() {
            let expected_format = format[index];
            let actual_format = ImageService::get_image_format(file);
            assert_eq!(expected_format, actual_format)
        }
    }
}