//! `Member` element related methods and entities.

use std::collections::HashMap;

use actix_web::{
    web::{Data, Json, Path},
    HttpResponse,
};
use futures::Future;
use medea::api::control::grpc::protos::control::{
    Member as MemberProto, Room_Element as RoomElementProto,
};
use serde::{Deserialize, Serialize};

use crate::{
    client::MemberUri,
    prelude::*,
    server::{endpoint::Endpoint, Context, Response, SingleGetResponse},
};

/// Path to member in REST API.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Deserialize)]
pub struct MemberPath {
    pub room_id: String,
    pub member_id: String,
}

/// `DELETE /{room_id}/{member_id}`
///
/// Delete single `Member`.
///
/// _For batch delete use `DELETE /`._
#[allow(clippy::needless_pass_by_value)]
pub fn delete(
    path: Path<MemberPath>,
    state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .delete_single(MemberUri::from(path))
        .map_err(|e| error!("{:?}", e))
        .map(|r| Response::from(r).into())
}

/// Entity that represents control API `Member`.
#[derive(Deserialize, Serialize, Debug)]
pub struct Member {
    /// Pipeline of control API `Member`.
    pipeline: HashMap<String, Endpoint>,
}

impl Into<MemberProto> for Member {
    fn into(self) -> MemberProto {
        let mut proto = MemberProto::new();
        let mut memebers_elements = HashMap::new();
        for (id, endpoint) in self.pipeline {
            memebers_elements.insert(id, endpoint.into());
        }
        proto.set_pipeline(memebers_elements);
        // TODO when credentials will generate on server, remove this.
        proto.set_credentials("test".to_string());

        proto
    }
}

impl From<MemberProto> for Member {
    fn from(mut proto: MemberProto) -> Self {
        let mut member_pipeline = HashMap::new();
        for (id, endpoint) in proto.take_pipeline() {
            member_pipeline.insert(id, endpoint.into());
        }
        Self {
            pipeline: member_pipeline,
        }
    }
}

impl Into<RoomElementProto> for Member {
    fn into(self) -> RoomElementProto {
        let mut proto = RoomElementProto::new();
        proto.set_member(self.into());
        proto
    }
}

/// `POST /{room_id}/{member_id}`
///
/// Create new `Member`.
#[allow(clippy::needless_pass_by_value)]
pub fn create(
    path: Path<MemberPath>,
    state: Data<Context>,
    data: Json<Member>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .create_member(&path.into(), data.0)
        .map_err(|e| error!("{:?}", e))
        .map(|r| Response::from(r).into())
}

/// `GET /{room_id}/{member_id}`
///
/// Get single `Member`.
///
/// _For batch get use `GET /`._
#[allow(clippy::needless_pass_by_value)]
pub fn get(
    path: Path<MemberPath>,
    state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .get_single(MemberUri::from(path))
        .map_err(|e| error!("{:?}", e))
        .map(|r| SingleGetResponse::from(r).into())
}
