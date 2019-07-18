use actix_web::{
    web::{Data, Path},
    HttpResponse,
};
use futures::Future;
use serde::Deserialize;

use crate::{
    prelude::*,
    server::{Context, Response},
};

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Deserialize)]
pub struct MemberPath {
    room_id: String,
    member_id: String,
}

#[allow(clippy::needless_pass_by_value)]
pub fn delete(
    path: Path<MemberPath>,
    state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .delete_room(&path.room_id)
        .map(|r| Response::from(r).into())
        .map_err(|e| error!("{:?}", e))
}
