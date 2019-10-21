//! REST [Control API] mock server implementation.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

pub mod endpoint;
pub mod member;
pub mod room;

use std::collections::HashMap;

use actix_cors::Cors;
use actix_web::{
    middleware,
    web::{self, Data, Json},
    App, HttpResponse, HttpServer,
};
use clap::ArgMatches;
use futures::Future;
use medea_control_api_proto::grpc::api::{
    CreateResponse as CreateResponseProto, Element as ElementProto,
    Error as ErrorProto, GetResponse as GetResponseProto,
    Response as ResponseProto, Room_Element as RoomElementProto,
};
use serde::{Deserialize, Serialize};

use crate::{
    client::{ControlClient, Uri},
    prelude::*,
};

use self::{
    endpoint::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
    member::Member,
    room::Room,
};

/// Context of [`actix_web`] server.
pub struct Context {
    /// Client for Medea's Control API.
    client: ControlClient,
}

/// Run REST Control API server mock.
pub fn run(args: &ArgMatches) {
    let medea_addr: String = args.value_of("medea_addr").unwrap().to_string();
    HttpServer::new(move || {
        App::new()
            .wrap(Cors::new())
            .data(Context {
                client: ControlClient::new(&medea_addr),
            })
            .wrap(middleware::Logger::default())
            .service(
                web::resource("/control-api/")
                    .route(web::post().to_async(create::create0))
                    .route(web::get().to_async(get::get0))
                    .route(web::delete().to_async(delete::delete0)),
            )
            .service(
                web::resource("/control-api/{a}")
                    .route(web::post().to_async(create::create1))
                    .route(web::get().to_async(get::get1))
                    .route(web::delete().to_async(delete::delete1)),
            )
            .service(
                web::resource("/control-api/{a}/{b}")
                    .route(web::post().to_async(create::create2))
                    .route(web::get().to_async(get::get2))
                    .route(web::delete().to_async(delete::delete2)),
            )
            .service(
                web::resource("/control-api/{a}/{b}/{c}")
                    .route(web::post().to_async(create::create3))
                    .route(web::get().to_async(get::get3))
                    .route(web::delete().to_async(delete::delete3)),
            )
    })
    .bind(args.value_of("addr").unwrap())
    .unwrap()
    .start();
}

macro_rules! gen_request_macro {
    ($call_fn:tt, $resp:ty) => {
        macro_rules! request {
            ($name:tt, $uri_tuple:ty) => {
                pub fn $name(
                    path: actix_web::web::Path<$uri_tuple>,
                    state: Data<Context>,
                ) -> impl Future<Item = HttpResponse, Error = ()> {
                    state
                        .client
                        .$call_fn(path.into_inner().into())
                        .map_err(|e| error!("{:?}", e))
                        .map(|r| <$resp>::from(r).into())
                }
            };
        }
    };
}

#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::module_name_repetitions)]
mod delete {
    use super::*;

    gen_request_macro!(delete, Response);

    request!(delete0, ());
    request!(delete1, String);
    request!(delete2, (String, String));
    request!(delete3, (String, String, String));
}

#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::module_name_repetitions)]
mod get {
    use super::*;

    gen_request_macro!(get, SingleGetResponse);

    request!(get0, ());
    request!(get1, String);
    request!(get2, (String, String));
    request!(get3, (String, String, String));
}

mod create {
    use super::*;

    macro_rules! create_request {
        ($fn_name:tt, $uri_tuple:ty) => {
            pub fn $fn_name(
                path: actix_web::web::Path<$uri_tuple>,
                state: Data<Context>,
                data: Json<Element>,
            ) -> impl Future<Item = HttpResponse, Error = ()> {
                state
                    .client
                    .create(Uri::from(path.into_inner()), data.0)
                    .map_err(|e| error!("{:?}", e))
                    .map(|r| CreateResponse::from(r).into())
            }
        };
    }

    create_request!(create0, ());
    create_request!(create1, (String));
    create_request!(create2, (String, String));
    create_request!(create3, (String, String, String));
}

/// Error object. Returns when some error happened on Control API's side.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Medea's Control API error code.
    pub code: u32,

    /// Text of error.
    pub text: String,

    /// Element's ID with which error happened.
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

/// Response which return sids.
///
/// Used for create methods.
#[derive(Debug, Serialize)]
pub struct CreateResponse {
    /// URIs with which Jason can connect `Member`s.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sids: Option<HashMap<String, String>>,

    /// Error if something happened on Control API's side.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorResponse>,
}

/// Response which can return only error (if any).
///
/// Used for delete methods.
#[derive(Debug, Serialize)]
pub struct Response {
    /// Error if something happened on Control API's side.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorResponse>,
}

/// Macro which implements [`Into`] [`HttpResponse`] for all
/// `control-api-mock` responses.
///
/// Implementation will check existence of `error` and if it exists then
/// [`HttpResponse`] will be `BadRequest` with this struct as response in
/// otherwise `Ok` with this struct as response.
macro_rules! impl_into_http_response {
    ($resp:tt) => {
        impl Into<HttpResponse> for $resp {
            fn into(self) -> HttpResponse {
                if self.error.is_some() {
                    HttpResponse::BadRequest().json(self)
                } else {
                    HttpResponse::Ok().json(self)
                }
            }
        }
    };
}

impl_into_http_response!(CreateResponse);
impl_into_http_response!(Response);
impl_into_http_response!(SingleGetResponse);

impl From<ResponseProto> for Response {
    fn from(mut resp: ResponseProto) -> Self {
        if resp.has_error() {
            Self {
                error: Some(resp.take_error().into()),
            }
        } else {
            Self { error: None }
        }
    }
}

impl From<CreateResponseProto> for CreateResponse {
    fn from(mut resp: CreateResponseProto) -> Self {
        if resp.has_error() {
            Self {
                sids: None,
                error: Some(resp.take_error().into()),
            }
        } else {
            Self {
                sids: Some(resp.take_sid()),
                error: None,
            }
        }
    }
}

/// Union of all elements which exists in medea.
#[derive(Deserialize, Serialize, Debug)]
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
            Self::Room(proto.take_room().into())
        } else if proto.has_member() {
            Self::Member(proto.take_member().into())
        } else if proto.has_webrtc_pub() {
            Self::WebRtcPublishEndpoint(proto.take_webrtc_pub().into())
        } else if proto.has_webrtc_play() {
            Self::WebRtcPlayEndpoint(proto.take_webrtc_play().into())
        } else {
            unimplemented!()
        }
    }
}

impl From<RoomElementProto> for Element {
    fn from(mut proto: RoomElementProto) -> Self {
        if proto.has_member() {
            Self::Member(proto.take_member().into())
        } else {
            unimplemented!()
        }
    }
}

impl Into<RoomElementProto> for Element {
    fn into(self) -> RoomElementProto {
        let mut proto = RoomElementProto::new();
        match self {
            Self::Member(m) => proto.set_member(m.into()),
            _ => unimplemented!(),
        }
        proto
    }
}

/// Response on request for get single `Element`s.
#[derive(Serialize, Debug)]
pub struct SingleGetResponse {
    /// Requested element.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<Element>,

    /// `Some(ErrorResponse)` if some error happened on Control API's side.
    /// Otherwise `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorResponse>,
}

impl From<GetResponseProto> for SingleGetResponse {
    fn from(mut proto: GetResponseProto) -> Self {
        if proto.has_error() {
            Self {
                element: None,
                error: Some(proto.take_error().into()),
            }
        } else {
            Self {
                error: None,
                element: proto
                    .take_elements()
                    .into_iter()
                    .map(|(_, e)| e.into())
                    .next(),
            }
        }
    }
}
