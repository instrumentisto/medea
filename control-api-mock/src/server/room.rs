use actix_web::{
    web::{Data, Path},
    HttpResponse,
};
use futures::Future;
use medea::api::control::grpc::protos::control::{
    Room as RoomProto, Room_Element as RoomElementProto,
};
use serde::{Deserialize, Serialize};

use super::Context;

use crate::{
    prelude::*,
    server::{member::Member, Response},
};
use actix_web::web::Json;
use std::collections::HashMap;

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Deserialize)]
pub struct RoomPath {
    pub room_id: String,
}

#[allow(clippy::needless_pass_by_value)]
pub fn delete(
    path: Path<RoomPath>,
    state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .delete_room(path.into())
        .map(|r| Response::from(r).into())
        .map_err(|e| error!("{:?}", e))
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Room {
    pipeline: HashMap<String, Member>,
}

impl Into<RoomProto> for Room {
    fn into(self) -> RoomProto {
        let mut proto = RoomProto::new();
        let mut room_elements = HashMap::new();
        for (id, member) in self.pipeline {
            room_elements.insert(id, member.into());
        }
        proto.set_pipeline(room_elements);

        proto
    }
}

pub fn create(
    path: Path<RoomPath>,
    state: Data<Context>,
    data: Json<Room>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .create_room(path.into(), data.0)
        .map(|r| Response::from(r).into())
        .map_err(|e| error!("{:?}", e))
}
