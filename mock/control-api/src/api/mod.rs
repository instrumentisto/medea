//! REST [Control API] mock server implementation.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

pub mod endpoint;
pub mod member;
pub mod room;

use std::collections::HashMap;

use actix::Addr;
use actix_cors::Cors;
use actix_web::{
    middleware,
    web::{self, Data, Json},
    App, HttpResponse, HttpServer,
};
use clap::ArgMatches;
use medea_control_api_proto::grpc::api as proto;
use serde::{Deserialize, Serialize};

use crate::{
    callback::server::{GetCallbackItems, GrpcCallbackServer},
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

    /// gRPC server which receives Control API callbacks.
    callback_server: Addr<GrpcCallbackServer>,
}

/// Run REST [Control API] server mock.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
pub async fn run(
    args: &ArgMatches<'static>,
    callback_server_addr: Addr<GrpcCallbackServer>,
) {
    let medea_addr: String = args.value_of("medea_addr").unwrap().to_string();
    let client = ControlClient::new(medea_addr).await.unwrap();
    HttpServer::new(move || {
        debug!("Running HTTP server...");
        App::new()
            .wrap(Cors::new().finish())
            .data(Context {
                client: client.clone(),
                callback_server: callback_server_addr.clone(),
            })
            .wrap(middleware::Logger::default())
            .service(
                web::resource("/control-api/{a}")
                    .route(web::post().to(create::create1))
                    .route(web::get().to(get::get1))
                    .route(web::delete().to(delete::delete1)),
            )
            .service(
                web::resource("/control-api/{a}/{b}")
                    .route(web::post().to(create::create2))
                    .route(web::get().to(get::get2))
                    .route(web::delete().to(delete::delete2)),
            )
            .service(
                web::resource("/control-api/{a}/{b}/{c}")
                    .route(web::post().to(create::create3))
                    .route(web::get().to(get::get3))
                    .route(web::delete().to(delete::delete3)),
            )
            .service(
                web::resource("/callbacks").route(web::get().to(get_callbacks)),
            )
    })
    .bind(args.value_of("addr").unwrap())
    .unwrap()
    .run()
    .await
    .unwrap();
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
                pub async fn $name(
                    path: actix_web::web::Path<$uri_tuple>,
                    state: Data<Context>,
                ) -> Result<HttpResponse, ()> {
                    state
                        .client
                        .$call_fn(path.into_inner().into())
                        .await
                        .map_err(|e| error!("{:?}", e))
                        .map(|r| <$resp>::from(r).into())
                }
            };
        }
    };
}

/// [`actix_web`] REST API endpoint which returns all
/// [`Callback`]s received by this mock server.
#[allow(clippy::needless_pass_by_value)]
pub async fn get_callbacks(state: Data<Context>) -> Result<HttpResponse, ()> {
    state
        .callback_server
        .send(GetCallbackItems)
        .await
        .map_err(|e| warn!("GrpcCallbackServer mailbox error. {:?}", e))
        .map(|callbacks| HttpResponse::Ok().json(&callbacks.unwrap()))
}

/// Implementation of `Delete` requests to [Control API] mock.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[allow(clippy::needless_pass_by_value)]
mod delete {
    use super::*;

    gen_request_macro!(delete, Response);

    request!(delete1, String);
    request!(delete2, (String, String));
    request!(delete3, (String, String, String));
}

/// Implementation of `Get` requests to [Control API] mock.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[allow(clippy::needless_pass_by_value)]
mod get {
    use super::*;

    gen_request_macro!(get, SingleGetResponse);

    request!(get1, String);
    request!(get2, (String, String));
    request!(get3, (String, String, String));
}

/// Implementation of `Post` requests to [Control API] mock.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[allow(clippy::needless_pass_by_value)]
mod create {
    use super::*;

    pub async fn create1(
        path: actix_web::web::Path<String>,
        state: Data<Context>,
        data: Json<Element>,
    ) -> Result<HttpResponse, ()> {
        state
            .client
            .create(path.into_inner(), Fid::from(()), data.0)
            .await
            .map_err(|e| error!("{:?}", e))
            .map(|r| CreateResponse::from(r).into())
    }

    pub async fn create2(
        path: actix_web::web::Path<(String, String)>,
        state: Data<Context>,
        data: Json<Element>,
    ) -> Result<HttpResponse, ()> {
        let uri = path.into_inner();
        state
            .client
            .create(uri.1, Fid::from(uri.0), data.0)
            .await
            .map_err(|e| error!("{:?}", e))
            .map(|r| CreateResponse::from(r).into())
    }

    pub async fn create3(
        path: actix_web::web::Path<(String, String, String)>,
        state: Data<Context>,
        data: Json<Element>,
    ) -> Result<HttpResponse, ()> {
        let uri = path.into_inner();
        state
            .client
            .create(uri.2, Fid::from((uri.0, uri.1)), data.0)
            .await
            .map_err(|e| error!("{:?}", e))
            .map(|r| CreateResponse::from(r).into())
    }
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

impl Into<ErrorResponse> for proto::Error {
    fn into(self) -> ErrorResponse {
        ErrorResponse {
            code: self.code,
            text: self.text,
            element: self.element,
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

impl From<proto::Response> for Response {
    fn from(resp: proto::Response) -> Self {
        Self {
            error: resp.error.map(|e| e.into()),
        }
    }
}

impl From<proto::CreateResponse> for CreateResponse {
    fn from(resp: proto::CreateResponse) -> Self {
        if let Some(error) = resp.error {
            Self {
                sids: None,
                error: Some(error.into()),
            }
        } else {
            Self {
                sids: Some(resp.sid),
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

impl Element {
    #[must_use]
    pub fn into_proto(self, id: String) -> proto::room::Element {
        let el = match self {
            Self::Member(m) => {
                proto::room::element::El::Member(m.into_proto(id))
            }
            _ => unimplemented!(),
        };
        proto::room::Element { el: Some(el) }
    }
}

impl From<proto::Element> for Element {
    fn from(proto: proto::Element) -> Self {
        use proto::element::El::*;

        match proto.el.unwrap() {
            Room(room) => Self::Room(room.into()),
            Member(member) => Self::Member(member.into()),
            WebrtcPub(webrtc_pub) => {
                Self::WebRtcPublishEndpoint(webrtc_pub.into())
            }
            WebrtcPlay(webrtc_play) => {
                Self::WebRtcPlayEndpoint(webrtc_play.into())
            }
        }
    }
}

impl From<proto::room::Element> for Element {
    fn from(proto: proto::room::Element) -> Self {
        match proto.el.unwrap() {
            proto::room::element::El::Member(member) => {
                Self::Member(member.into())
            }
            _ => unimplemented!(
                "Currently Control API mock server supports only Member \
                 element in Room pipeline."
            ),
        }
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

impl From<proto::GetResponse> for SingleGetResponse {
    fn from(proto: proto::GetResponse) -> Self {
        if let Some(error) = proto.error {
            Self {
                element: None,
                error: Some(error.into()),
            }
        } else {
            Self {
                error: None,
                element: proto
                    .elements
                    .into_iter()
                    .map(|(_, e)| e.into())
                    .next(),
            }
        }
    }
}
