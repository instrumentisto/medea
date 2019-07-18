mod endpoint;
mod member;
mod room;

use std::collections::HashMap;

use actix_web::{middleware, App, HttpResponse, HttpServer};
use medea::api::control::grpc::protos::control::{
    Error as ErrorProto, Response as ResponseProto,
};
use serde::Serialize;

use crate::client::ControlClient;

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
                actix_web::web::resource("/{room_id}")
                    .route(actix_web::web::delete().to_async(room::delete)),
            )
            .service(
                actix_web::web::resource("/{room_id}/{member_id}")
                    .route(actix_web::web::delete().to_async(member::delete)),
            )
            .service(
                actix_web::web::resource(
                    "/{room_id}/{member_id}/{endpoint_id}",
                )
                .route(actix_web::web::delete().to_async(endpoint::delete)),
            )
    })
    .bind("0.0.0.0:8000")
    .unwrap()
    .start();
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
