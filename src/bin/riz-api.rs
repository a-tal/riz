use std::{env, error::Error, net::Ipv4Addr, sync::Mutex};

use actix_cors::Cors;
use actix_web::{http::header, middleware::Logger, web::Data, App, HttpServer, Result};
use log::info;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use riz::{health, lights, models, rooms, Storage, Worker};

fn get_port() -> u16 {
    let port = env::var("RIZ_PORT").unwrap_or(String::from("8080"));
    match u16::from_str_radix(&port, 10) {
        Ok(v) => v,
        Err(e) => {
            log::error!("Invalid port: {port}: {:?}", e);
            8080
        }
    }
}

#[actix_web::main]
async fn main() -> Result<(), impl Error> {
    env::set_var("RUST_LOG", "debug");
    env_logger::init();

    #[derive(OpenApi)]
    #[openapi(
        paths(
            health::ping,
            rooms::create,
            rooms::list,
            rooms::read,
            rooms::update,
            rooms::destroy,
            rooms::status,
            lights::create,
            lights::update,
            lights::destroy,
            lights::update_room,
            lights::update_light,
            lights::status,
        ),
        components(schemas(
            models::Room,
            models::Light,
            models::LightRequest,
            models::LightStatus,
            models::PowerMode,
            models::SceneMode,
            models::Brightness,
            models::Color,
            models::Kelvin,
            models::White,
            models::Speed,
            models::LastSet,
        ))
    )]
    struct ApiDoc;

    let openapi = ApiDoc::openapi();

    let storage = Data::new(Mutex::new(Storage::new()));
    let worker = Data::new(Mutex::new(Worker::new(Data::clone(&storage))));

    let port = get_port();
    info!("Listening on port: {port}");

    HttpServer::new(move || {
        let origin = match env::var("RIZ_CORS_ORIGIN") {
            Ok(val) => val,
            Err(_) => String::from("http://localhost:8000"),
        };
        let origin = origin.as_str();

        let cors = Cors::default()
            .allowed_origin(origin)
            .allow_any_method()
            .allowed_header(header::CONTENT_TYPE)
            .max_age(600);

        App::new()
            .wrap(cors)
            .app_data(Data::clone(&storage))
            .app_data(Data::clone(&worker))
            .wrap(Logger::default())
            .service(rooms::create)
            .service(rooms::list)
            .service(rooms::read)
            .service(rooms::update)
            .service(rooms::destroy)
            .service(rooms::status)
            .service(lights::create)
            .service(lights::update)
            .service(lights::update_room)
            .service(lights::update_light)
            .service(lights::destroy)
            .service(lights::status)
            .service(health::ping)
            .service(
                SwaggerUi::new("/v1/swagger-ui/{_:.*}")
                    .url("/v1/api-docs/openapi.json", openapi.clone()),
            )
    })
    .bind((Ipv4Addr::UNSPECIFIED, port))?
    .run()
    .await
}
