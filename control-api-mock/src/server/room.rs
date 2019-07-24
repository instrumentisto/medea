//! `Room` element related methods and entities.

use std::collections::HashMap;

use actix_web::{
    web::{Data, Json, Path},
    HttpResponse,
};
use futures::Future;
use medea_grpc_proto::control::{
    Room as RoomProto, Room_Element as RoomElementProto,
};
use serde::{Deserialize, Serialize};

use crate::{
    client::RoomUri,
    prelude::*,
    server::{member::Member, Response, SingleGetResponse},
};

use super::Context;

/// Path to `Room` in REST API.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Deserialize)]
pub struct RoomPath {
    pub room_id: String,
}

/// `DELETE /{room_id}`
///
/// Delete room.
///
/// _For batch delete use `DELETE /`._
#[allow(clippy::needless_pass_by_value)]
pub fn delete(
    path: Path<RoomPath>,
    state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .delete_single(RoomUri::from(path))
        .map_err(|e| error!("{:?}", e))
        .map(|r| Response::from(r).into())
}

/// Control API's `Room` representation.
#[derive(Serialize, Deserialize, Debug)]
pub struct Room {
    /// Pipeline of `Room`.
    pipeline: HashMap<String, RoomElement>,
}

/// Element of [`Room`]'s pipeline.
#[allow(clippy::module_name_repetitions)]
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum RoomElement {
    Member(Member),
}

impl Into<RoomElementProto> for RoomElement {
    fn into(self) -> RoomElementProto {
        let mut proto = RoomElementProto::new();
        match self {
            RoomElement::Member(m) => proto.set_member(m.into()),
        }

        proto
    }
}

impl From<RoomElementProto> for RoomElement {
    fn from(mut proto: RoomElementProto) -> Self {
        if proto.has_member() {
            RoomElement::Member(proto.take_member().into())
        } else {
            unimplemented!()
        }
    }
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

impl From<RoomProto> for Room {
    fn from(mut proto: RoomProto) -> Self {
        let mut pipeline = HashMap::new();
        for (id, member) in proto.take_pipeline() {
            pipeline.insert(id, member.into());
        }
        Self { pipeline }
    }
}

/// `POST /{room_id}`
///
/// Create new `Room`.
#[allow(clippy::needless_pass_by_value)]
pub fn create(
    path: Path<RoomPath>,
    state: Data<Context>,
    data: Json<Room>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .create_room(&path.into(), data.0)
        .map_err(|e| error!("{:?}", e))
        .map(|r| Response::from(r).into())
}

/// `GET /{room_id}`
///
/// Get single `Room`.
///
/// _For batch get use `GET /`._
#[allow(clippy::needless_pass_by_value)]
pub fn get(
    path: Path<RoomPath>,
    state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .get_single(RoomUri::from(path))
        .map_err(|e| error!("{:?}", e))
        .map(|r| SingleGetResponse::from(r).into())
}
