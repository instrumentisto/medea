mod endpoint;
mod member;
mod room;

use actix_web::{middleware, App, HttpServer};

use crate::client::ControlClient;

pub struct Context {
    client: ControlClient,
}

pub fn run() {
    HttpServer::new(|| {
        App::new()
            .data(Context {
                client: ControlClient::new(),
            })
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
