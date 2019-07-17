use actix_web::{
    web::{Data, Path},
    Error, HttpResponse,
};
use futures::Future;
use serde::{Deserialize, Serialize};

use super::Context;

#[derive(Debug, Deserialize)]
pub struct RoomPath {
    room_id: String,
}

#[derive(Debug, Serialize)]
pub struct HelloWorld {
    text: String,
}

pub fn delete_room(
    path: Path<RoomPath>,
    _state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    let hello = HelloWorld {
        text: format!("Hello world. Room id: {}", path.room_id),
    };
    futures::future::ok(HttpResponse::Ok().json(hello))
}
