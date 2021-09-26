# Image Resizer
Resizing image service backed by s3.

Resized images are cached in the specificed s3 bucket.

## Running
Expects and s3 bucket with two folder `images/` and `cache/`.

As well as a aws .credentials file setup with access too the s3 bucket.

```
AWS_ROLE=<credential_role> IMAGE_BUCKET=<s3_bucket_name> ./image-resizer
```

## Routes

`POST /upload`: accepts upload of file with content type jpeg, png, ico, gif

`GET /_list`: lists images uploaded to s3 bucket, private

`GET /{image_file_name}`: returns image which is optionally resizeable with the following variants where w is width & h is height

- `GET /{image_file_name}?w=<num>`

- `GET /{image_file_name}?h=<num>`

- `GET /{image_file_name}?w=<num>&h=<num>`
