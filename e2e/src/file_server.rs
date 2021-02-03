use std::{convert::TryFrom as _, path::PathBuf};

use futures::{channel::oneshot, future::select};
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, Server, StatusCode,
};
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};

const NOT_FOUND: &[u8] = b"Not found";

pub struct FileServer(Option<oneshot::Sender<()>>);

impl FileServer {
    pub fn run() -> Self {
        let make_service = make_service_fn(|_| async {
            Ok::<_, hyper::Error>(service_fn(response_files))
        });
        let server =
            Server::bind(&"0.0.0.0:30000".parse().unwrap()).serve(make_service);

        let (tx, rx) = oneshot::channel();
        tokio::spawn(select(server, rx));

        Self(Some(tx))
    }
}

impl Drop for FileServer {
    fn drop(&mut self) {
        if let Some(tx) = self.0.take() {
            let _ = tx.send(());
        }
    }
}

fn not_found() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(NOT_FOUND.into())
        .unwrap()
}

async fn response_files(
    req: Request<Body>,
) -> Result<Response<Body>, hyper::Error> {
    let path = &req.uri().path()[1..];
    let path: PathBuf = PathBuf::try_from(path).unwrap();

    if !(path.starts_with("jason") || path.starts_with("index.html")) {
        return Ok(not_found());
    }

    let mime = match path.extension().unwrap().to_str().unwrap() {
        "js" => "text/javascript",
        "html" => "text/html",
        "wasm" => "application/wasm",
        _ => panic!(),
    };

    if req.method() == Method::GET {
        if let Ok(file) = File::open(path).await {
            let stream = FramedRead::new(file, BytesCodec::new());
            let body = Body::wrap_stream(stream);

            return Ok(Response::builder()
                .header("Content-Type", mime)
                .body(body)
                .unwrap());
        }
    }

    Ok(not_found())
}
