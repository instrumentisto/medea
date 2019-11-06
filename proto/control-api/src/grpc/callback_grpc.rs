// This file is generated. Do not edit
// @generated

// https://github.com/Manishearth/rust-clippy/issues/702
#![allow(unknown_lints)]
#![allow(clippy)]

#![cfg_attr(rustfmt, rustfmt_skip)]

#![allow(box_pointers)]
#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(trivial_casts)]
#![allow(unsafe_code)]
#![allow(unused_imports)]
#![allow(unused_results)]

const METHOD_CALLBACK_ON_EVENT: ::grpcio::Method<super::callback::Request, super::callback::Response> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/medea_callback.Callback/OnEvent",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

#[derive(Clone)]
pub struct CallbackClient {
    client: ::grpcio::Client,
}

impl CallbackClient {
    pub fn new(channel: ::grpcio::Channel) -> Self {
        CallbackClient {
            client: ::grpcio::Client::new(channel),
        }
    }

    pub fn on_event_opt(&self, req: &super::callback::Request, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::callback::Response> {
        self.client.unary_call(&METHOD_CALLBACK_ON_EVENT, req, opt)
    }

    pub fn on_event(&self, req: &super::callback::Request) -> ::grpcio::Result<super::callback::Response> {
        self.on_event_opt(req, ::grpcio::CallOption::default())
    }

    pub fn on_event_async_opt(&self, req: &super::callback::Request, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::callback::Response>> {
        self.client.unary_call_async(&METHOD_CALLBACK_ON_EVENT, req, opt)
    }

    pub fn on_event_async(&self, req: &super::callback::Request) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::callback::Response>> {
        self.on_event_async_opt(req, ::grpcio::CallOption::default())
    }
    pub fn spawn<F>(&self, f: F) where F: ::futures::Future<Item = (), Error = ()> + Send + 'static {
        self.client.spawn(f)
    }
}

pub trait Callback {
    fn on_event(&mut self, ctx: ::grpcio::RpcContext, req: super::callback::Request, sink: ::grpcio::UnarySink<super::callback::Response>);
}

pub fn create_callback<S: Callback + Send + Clone + 'static>(s: S) -> ::grpcio::Service {
    let mut builder = ::grpcio::ServiceBuilder::new();
    let mut instance = s;
    builder = builder.add_unary_handler(&METHOD_CALLBACK_ON_EVENT, move |ctx, req, resp| {
        instance.on_event(ctx, req, resp)
    });
    builder.build()
}
