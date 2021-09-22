use actix_web::{get, App, HttpServer};

#[get("/")]
async fn hello() -> &'static str {
    "Hello world, rust!\r\n"
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(hello)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
