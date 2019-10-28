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
    Element_oneof_el as ElementOneOf, Error as ErrorProto,
    GetResponse as GetResponseProto, Response as ResponseProto,
    Room_Element as RoomElementProto,
    Room_Element_oneof_el as RoomElementOneOf,
};
use serde::{Deserialize, Serialize};

use crate::{
    client::{ControlClient, Fid},
    prelude::*,
};

use self::{
    endpoint::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
    member::Member,
    room::Room,
};

/// Context of [`actix_web`] server.
pub struct Context {
    /// Client for [Medea]'s [Control API].
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    /// [Medea]: https://github.com/instrumentisto/medea
    client: ControlClient,
}

/// Run REST [Control API] server mock.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
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

/// Generates `request` macro which will generate [`actix_web`] request handler
/// which will call some function with `Path` extracted from `Request`.
///
/// `$call_fn` - function which will be called on request;
///
/// `$resp` - type of response on this request.
macro_rules! gen_request_macro {
    ($call_fn:tt, $resp:ty) => {
        /// Generates handler with provided name and `Path` which will be
        /// passed to `$call_fn` function.
        ///
        /// `$name` - name of generated function;
        ///
        /// `$uri_tuple` - type of path which will be provided by [`actix_web`].
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

/// Implementation of `Delete` requests to [Control API] mock.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
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

/// Implementation of `Get` requests to [Control API] mock.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
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

/// Implementation of `Post` requests to [Control API] mock.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
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
                    .create(Fid::from(path.into_inner()), data.0)
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

/// Error object. Returns when some error happened on [Control API]'s side.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
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

/// Response which returns sids.
///
/// Used for create methods.
#[derive(Debug, Serialize)]
pub struct CreateResponse {
    /// URIs with which [Jason] can connect `Member`s.
    ///
    /// [Jason]: https://github.com/instrumentisto/medea/tree/master/jason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sids: Option<HashMap<String, String>>,

    /// Error if something happened on [Control API]'s side.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorResponse>,
}

/// Response which can return only error (if any).
///
/// Used for delete methods.
#[derive(Debug, Serialize)]
pub struct Response {
    /// Error if something happened on [Control API]'s side.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
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

/// Union of all elements which exists in [Medea].
///
/// [Medea]: https://github.com/instrumentisto/medea
#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "kind")]
pub enum Element {
    Member(Member),
    WebRtcPublishEndpoint(WebRtcPublishEndpoint),
    WebRtcPlayEndpoint(WebRtcPlayEndpoint),
    Room(Room),
}

impl From<ElementProto> for Element {
    fn from(proto: ElementProto) -> Self {
        match proto.el.unwrap() {
            ElementOneOf::room(room) => Self::Room(room.into()),
            ElementOneOf::member(member) => Self::Member(member.into()),
            ElementOneOf::webrtc_pub(webrtc_pub) => {
                Self::WebRtcPublishEndpoint(webrtc_pub.into())
            }
            ElementOneOf::webrtc_play(webrtc_play) => {
                Self::WebRtcPlayEndpoint(webrtc_play.into())
            }
        }
    }
}

impl From<RoomElementProto> for Element {
    fn from(proto: RoomElementProto) -> Self {
        match proto.el.unwrap() {
            RoomElementOneOf::member(member) => Self::Member(member.into()),
            _ => unimplemented!(
                "Currently Control API mock server supports only Member \
                 element in Room pipeline."
            ),
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

/// Response on request for get `Element` request.
#[derive(Debug, Serialize)]
pub struct SingleGetResponse {
    /// Requested element.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<Element>,

    /// [`ErrorResponse`] if some error happened on [Control API]'s side.
    /// Otherwise `None`.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
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
