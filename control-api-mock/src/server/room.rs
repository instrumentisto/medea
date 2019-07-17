use actix_web::{
    web::{Data, Path},
    Error, HttpResponse,
};
use futures::Future;
use serde::{Deserialize, Serialize};

use super::Context;

use crate::prelude::*;

#[derive(Debug, Deserialize)]
pub struct RoomPath {
    room_id: String,
}

#[derive(Debug, Serialize)]
pub struct HelloWorld {}

pub fn delete_room(
    path: Path<RoomPath>,
    state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .delete_room(path.room_id.clone())
        .map(|r| HttpResponse::Ok().json(HelloWorld {}))
        .map_err(|e| error!("{:?}", e))
}
