pub mod endpoint;
pub mod member;
pub mod room;

use std::collections::HashMap;

use actix_web::{
    middleware,
    web::{self, Data, Json},
    App, HttpResponse, HttpServer,
};
use futures::Future;
use medea::api::control::grpc::protos::control::{
    Element as ElementProto, Error as ErrorProto,
    GetResponse as GetResponseProto, Response as ResponseProto,
    Room_Element as RoomElementProto,
};
use serde::{Deserialize, Serialize};

use crate::{
    client::ControlClient,
    prelude::*,
    server::{
        endpoint::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
        member::Member,
        room::Room,
    },
};

pub struct Context {
    client: ControlClient,
}

pub fn run() {
    HttpServer::new(|| {
        App::new()
            .data(Context {
                client: ControlClient::new(),
            })
            .wrap(middleware::Logger::default())
            .service(
                web::resource("/")
                    .route(web::get().to_async(batch_get))
                    .route(web::delete().to_async(batch_delete)),
            )
            .service(
                web::resource("/{room_id}")
                    .route(web::delete().to_async(room::delete))
                    .route(web::post().to_async(room::create))
                    .route(web::get().to_async(room::get)),
            )
            .service(
                web::resource("/{room_id}/{member_id}")
                    .route(web::delete().to_async(member::delete))
                    .route(web::post().to_async(member::create))
                    .route(web::get().to_async(member::get)),
            )
            .service(
                web::resource("/{room_id}/{member_id}/{endpoint_id}")
                    .route(web::delete().to_async(endpoint::delete))
                    .route(web::post().to_async(endpoint::create))
                    .route(web::get().to_async(endpoint::get)),
            )
    })
    .bind("0.0.0.0:8000")
    .unwrap()
    .start();
}

#[derive(Deserialize, Debug)]
pub struct BatchIdsRequest {
    ids: Vec<String>,
}

#[allow(clippy::needless_pass_by_value)]
pub fn batch_get(
    state: Data<Context>,
    data: Json<BatchIdsRequest>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .get_batch(data.ids.clone())
        .map_err(|e| error!("{:?}", e))
        .map(|r| GetResponse::from(r).into())
}

#[allow(clippy::needless_pass_by_value)]
pub fn batch_delete(
    state: Data<Context>,
    data: Json<BatchIdsRequest>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .delete_batch(data.0.ids)
        .map_err(|e| error!("{:?}", e))
        .map(|r| Response::from(r).into())
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub code: u32,
    pub text: String,
    pub element: String,
}

impl Into<ErrorResponse> for ErrorProto {
    fn into(mut self) -> ErrorResponse {
        ErrorResponse {
            code: self.get_code(),
            text: self.take_text(),
            element: self.take_element(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Response {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sid: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorResponse>,
}

impl Into<HttpResponse> for Response {
    fn into(self) -> HttpResponse {
        if self.error.is_some() {
            HttpResponse::BadRequest().json(self)
        } else {
            HttpResponse::Ok().json(self)
        }
    }
}

impl From<ResponseProto> for Response {
    fn from(mut resp: ResponseProto) -> Self {
        if resp.has_error() {
            Self {
                sid: None,
                error: Some(resp.take_error().into()),
            }
        } else {
            Self {
                sid: Some(resp.take_sid()),
                error: None,
            }
        }
    }
}

#[derive(Serialize, Debug)]
#[serde(tag = "kind")]
pub enum Element {
    Member(Member),
    WebRtcPublishEndpoint(WebRtcPublishEndpoint),
    WebRtcPlayEndpoint(WebRtcPlayEndpoint),
    Room(Room),
}

impl From<ElementProto> for Element {
    fn from(mut proto: ElementProto) -> Self {
        if proto.has_room() {
            Element::Room(proto.take_room().into())
        } else if proto.has_member() {
            Element::Member(proto.take_member().into())
        } else if proto.has_webrtc_pub() {
            Element::WebRtcPublishEndpoint(proto.take_webrtc_pub().into())
        } else if proto.has_webrtc_play() {
            Element::WebRtcPlayEndpoint(proto.take_webrtc_play().into())
        } else {
            unimplemented!()
        }
    }
}

impl From<RoomElementProto> for Element {
    fn from(mut proto: RoomElementProto) -> Self {
        if proto.has_member() {
            Element::Member(proto.take_member().into())
        } else {
            unimplemented!()
        }
    }
}

impl Into<RoomElementProto> for Element {
    fn into(self) -> RoomElementProto {
        let mut proto = RoomElementProto::new();
        match self {
            Element::Member(m) => proto.set_member(m.into()),
            _ => unimplemented!(),
        }
        proto
    }
}

#[derive(Serialize, Debug)]
pub struct GetResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elements: Option<HashMap<String, Element>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorResponse>,
}

impl From<GetResponseProto> for GetResponse {
    fn from(mut proto: GetResponseProto) -> Self {
        if proto.has_error() {
            return Self {
                elements: None,
                error: Some(proto.take_error().into()),
            };
        }

        let mut elements = HashMap::new();
        for (id, element) in proto.take_elements() {
            elements.insert(id, element.into());
        }

        Self {
            elements: Some(elements),
            error: None,
        }
    }
}

impl Into<HttpResponse> for GetResponse {
    fn into(self) -> HttpResponse {
        if self.error.is_some() {
            HttpResponse::BadRequest().json(self)
        } else {
            HttpResponse::Ok().json(self)
        }
    }
}
