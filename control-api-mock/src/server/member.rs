use actix_web::{
    web::{Data, Path},
    HttpResponse,
};
use futures::Future;
use medea::api::control::grpc::protos::control::Member as MemberProto;
use serde::{Deserialize, Serialize};

use crate::{
    prelude::*,
    server::{endpoint::Endpoint, Context, Response},
};
use actix_web::web::Json;
use std::collections::HashMap;

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Deserialize)]
pub struct MemberPath {
    pub room_id: String,
    pub member_id: String,
}

#[allow(clippy::needless_pass_by_value)]
pub fn delete(
    path: Path<MemberPath>,
    state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .delete_member(path.into())
        .map(|r| Response::from(r).into())
        .map_err(|e| error!("{:?}", e))
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Member {
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
        // TODO
        proto.set_credentials("test".to_string());

        proto
    }
}

pub fn create(
    path: Path<MemberPath>,
    state: Data<Context>,
    data: Json<Member>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .create_member(path.into(), data.0)
        .map(|r| Response::from(r).into())
        .map_err(|e| error!("{:?}", e))
}
