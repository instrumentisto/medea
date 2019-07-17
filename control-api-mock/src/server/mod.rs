mod endpoint;
mod member;
mod room;

use actix_web::{middleware, App, HttpServer};

pub struct Context {}

pub fn run() {
    HttpServer::new(|| {
        App::new()
            .data(Context {})
            .wrap(middleware::Logger::default())
            .service(
                actix_web::web::resource("/{room_id}").route(
                    actix_web::web::delete().to_async(room::delete_room),
                ),
            )
    })
    .bind("0.0.0.0:8000")
    .unwrap()
    .start();
}
