//! `Member` element related methods and entities.

use std::collections::HashMap;

use actix_web::{
    web::{Data, Json, Path},
    HttpResponse,
};
use futures::Future;
use medea_control_api_proto::grpc::control_api::{
    Member as MemberProto, Room_Element as RoomElementProto,
};
use serde::{Deserialize, Serialize};

use crate::{client::Uri, prelude::*};

use super::{endpoint::Endpoint, Context, CreateResponse};

/// Entity that represents control API `Member`.
#[derive(Deserialize, Serialize, Debug)]
pub struct Member {
    /// Pipeline of Control API `Member`.
    pipeline: HashMap<String, Endpoint>,

    /// Optional member credentials.
    ///
    /// If `None` then random credentials will be generated on Medea side.
    credentials: Option<String>,
}

impl Into<MemberProto> for Member {
    fn into(self) -> MemberProto {
        let mut proto = MemberProto::new();
        let mut memebers_elements = HashMap::new();
        for (id, endpoint) in self.pipeline {
            memebers_elements.insert(id, endpoint.into());
        }
        proto.set_pipeline(memebers_elements);

        if let Some(credentials) = self.credentials {
            proto.set_credentials(credentials);
        }

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
            credentials: Some(proto.take_credentials()),
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
/// Creates new `Member` element.
#[allow(clippy::needless_pass_by_value)]
pub fn create(
    path: Path<(String, String)>,
    state: Data<Context>,
    data: Json<Member>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .create_member(Uri::from(path.into_inner()), data.0)
        .map_err(|e| error!("{:?}", e))
        .map(|r| CreateResponse::from(r).into())
}
