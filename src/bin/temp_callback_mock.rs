use futures::{future::Future as _, sync::oneshot};
use grpcio::{Environment, RpcContext, ServerBuilder, UnarySink};
use medea_control_api_proto::grpc::{
    callback::{Request, Response},
    callback_grpc::{create_callback, Callback},
};
use std::{io, io::Read as _, sync::Arc, thread};

#[derive(Clone)]
struct CallbackService;

impl Callback for CallbackService {
    fn on_event(
        &mut self,
        ctx: RpcContext,
        req: Request,
        sink: UnarySink<Response>,
    ) {
        panic!("{:?}", req);

        ctx.spawn(sink.success(Response::new()).map_err(|_| ()))
    }
}

fn main() {
    let cq_count = 2;

    let service = create_callback(CallbackService);
    let env = Arc::new(Environment::new(cq_count));

    let mut server = ServerBuilder::new(env)
        .register_service(service)
        .bind("127.0.0.1".to_string(), 9099)
        .build()
        .unwrap();
    server.start();
    let (tx, rx) = oneshot::channel();
    thread::spawn(move || {
        let _ = io::stdin().read(&mut [0]).unwrap();
        tx.send(())
    });
    let _ = rx.wait();
    let _ = server.shutdown().wait();
}
