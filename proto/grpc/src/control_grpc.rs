// This file is generated. Do not edit
// @generated

// https://github.com/Manishearth/rust-clippy/issues/702
#![allow(unknown_lints)]
#![allow(clippy::all)]

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

const METHOD_CONTROL_API_CREATE: ::grpcio::Method<super::control::CreateRequest, super::control::Response> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/medea.ControlApi/Create",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_CONTROL_API_APPLY: ::grpcio::Method<super::control::ApplyRequest, super::control::Response> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/medea.ControlApi/Apply",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_CONTROL_API_DELETE: ::grpcio::Method<super::control::IdRequest, super::control::Response> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/medea.ControlApi/Delete",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_CONTROL_API_GET: ::grpcio::Method<super::control::IdRequest, super::control::GetResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/medea.ControlApi/Get",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

#[derive(Clone)]
pub struct ControlApiClient {
    client: ::grpcio::Client,
}

impl ControlApiClient {
    pub fn new(channel: ::grpcio::Channel) -> Self {
        ControlApiClient {
            client: ::grpcio::Client::new(channel),
        }
    }

    pub fn create_opt(&self, req: &super::control::CreateRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::control::Response> {
        self.client.unary_call(&METHOD_CONTROL_API_CREATE, req, opt)
    }

    pub fn create(&self, req: &super::control::CreateRequest) -> ::grpcio::Result<super::control::Response> {
        self.create_opt(req, ::grpcio::CallOption::default())
    }

    pub fn create_async_opt(&self, req: &super::control::CreateRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::control::Response>> {
        self.client.unary_call_async(&METHOD_CONTROL_API_CREATE, req, opt)
    }

    pub fn create_async(&self, req: &super::control::CreateRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::control::Response>> {
        self.create_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn apply_opt(&self, req: &super::control::ApplyRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::control::Response> {
        self.client.unary_call(&METHOD_CONTROL_API_APPLY, req, opt)
    }

    pub fn apply(&self, req: &super::control::ApplyRequest) -> ::grpcio::Result<super::control::Response> {
        self.apply_opt(req, ::grpcio::CallOption::default())
    }

    pub fn apply_async_opt(&self, req: &super::control::ApplyRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::control::Response>> {
        self.client.unary_call_async(&METHOD_CONTROL_API_APPLY, req, opt)
    }

    pub fn apply_async(&self, req: &super::control::ApplyRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::control::Response>> {
        self.apply_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn delete_opt(&self, req: &super::control::IdRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::control::Response> {
        self.client.unary_call(&METHOD_CONTROL_API_DELETE, req, opt)
    }

    pub fn delete(&self, req: &super::control::IdRequest) -> ::grpcio::Result<super::control::Response> {
        self.delete_opt(req, ::grpcio::CallOption::default())
    }

    pub fn delete_async_opt(&self, req: &super::control::IdRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::control::Response>> {
        self.client.unary_call_async(&METHOD_CONTROL_API_DELETE, req, opt)
    }

    pub fn delete_async(&self, req: &super::control::IdRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::control::Response>> {
        self.delete_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn get_opt(&self, req: &super::control::IdRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::control::GetResponse> {
        self.client.unary_call(&METHOD_CONTROL_API_GET, req, opt)
    }

    pub fn get(&self, req: &super::control::IdRequest) -> ::grpcio::Result<super::control::GetResponse> {
        self.get_opt(req, ::grpcio::CallOption::default())
    }

    pub fn get_async_opt(&self, req: &super::control::IdRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::control::GetResponse>> {
        self.client.unary_call_async(&METHOD_CONTROL_API_GET, req, opt)
    }

    pub fn get_async(&self, req: &super::control::IdRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::control::GetResponse>> {
        self.get_async_opt(req, ::grpcio::CallOption::default())
    }
    pub fn spawn<F>(&self, f: F) where F: ::futures::Future<Item = (), Error = ()> + Send + 'static {
        self.client.spawn(f)
    }
}

pub trait ControlApi {
    fn create(&mut self, ctx: ::grpcio::RpcContext, req: super::control::CreateRequest, sink: ::grpcio::UnarySink<super::control::Response>);
    fn apply(&mut self, ctx: ::grpcio::RpcContext, req: super::control::ApplyRequest, sink: ::grpcio::UnarySink<super::control::Response>);
    fn delete(&mut self, ctx: ::grpcio::RpcContext, req: super::control::IdRequest, sink: ::grpcio::UnarySink<super::control::Response>);
    fn get(&mut self, ctx: ::grpcio::RpcContext, req: super::control::IdRequest, sink: ::grpcio::UnarySink<super::control::GetResponse>);
}

pub fn create_control_api<S: ControlApi + Send + Clone + 'static>(s: S) -> ::grpcio::Service {
    let mut builder = ::grpcio::ServiceBuilder::new();
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_CONTROL_API_CREATE, move |ctx, req, resp| {
        instance.create(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_CONTROL_API_APPLY, move |ctx, req, resp| {
        instance.apply(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_CONTROL_API_DELETE, move |ctx, req, resp| {
        instance.delete(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_CONTROL_API_GET, move |ctx, req, resp| {
        instance.get(ctx, req, resp)
    });
    builder.build()
}
