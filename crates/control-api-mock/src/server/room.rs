//! `Room` element related methods and entities.

use std::collections::HashMap;

use actix_web::{
    web::{Data, Json, Path},
    HttpResponse,
};
use futures::Future;
use medea_control_api_proto::grpc::control_api::{
    Room as RoomProto, Room_Element as RoomElementProto,
};
use serde::{Deserialize, Serialize};

use crate::{client::Uri, prelude::*};

use super::{member::Member, Context, CreateResponse};

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
            Self::Member(m) => proto.set_member(m.into()),
        }

        proto
    }
}

impl From<RoomElementProto> for RoomElement {
    fn from(mut proto: RoomElementProto) -> Self {
        if proto.has_member() {
            Self::Member(proto.take_member().into())
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
/// Creates new `Room` element.
#[allow(clippy::needless_pass_by_value)]
pub fn create(
    path: Path<String>,
    state: Data<Context>,
    data: Json<Room>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .create_room(Uri::from(path.into_inner()), data.0)
        .map_err(|e| error!("{:?}", e))
        .map(|r| CreateResponse::from(r).into())
}
