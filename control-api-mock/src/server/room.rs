use actix_web::{
    web::{Data, Path},
    HttpResponse,
};
use futures::Future;
use serde::Deserialize;

use super::Context;

use crate::{prelude::*, server::Response};

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
