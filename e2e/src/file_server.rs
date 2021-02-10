//! Implementation of the HTTP file server which will share files required for
//! tests.

use std::path::PathBuf;

use futures::{channel::oneshot, future};
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, Server, StatusCode,
};
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};

use crate::conf;

/// File server which will share `index.html` and compiled `jason` library.
pub struct FileServer(oneshot::Sender<()>);

impl FileServer {
    /// Starts file server which will share `index.html` and compiled `jason`
    /// library.
    pub fn run() -> Self {
        let server = Server::bind(&conf::FILE_SERVER_ADDR.parse().unwrap())
            .serve(make_service_fn(|_| async {
                Ok::<_, hyper::Error>(service_fn(serve))
            }));

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        tokio::spawn(future::select(server, shutdown_rx));

        Self(shutdown_tx)
    }
}

/// Handles all files requests to this [`FileServer`].
async fn serve(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let path = &req.uri().path()[1..];
    let mut splitted_path = path.split('/');
    let first = splitted_path.next().unwrap_or("index.html");
    let path = match first {
        "jason" => {
            let mut path = PathBuf::from(&*conf::JASON_DIR_PATH);
            for p in splitted_path {
                path.push(p);
            }
            path
        }
        "index.html" => PathBuf::from(&*conf::INDEX_PATH),
        _ => {
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body("Unknown directory requested".into())
                .unwrap());
        }
    };

    if req.method() != Method::GET {
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Only GET method is expected".into())
            .unwrap());
    }

    let content_type = match path.extension().unwrap().to_str().unwrap() {
        "js" => "text/javascript",
        "html" => "text/html",
        "wasm" => "application/wasm",
        _ => {
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body("Only js, html and wasm files are expected".into())
                .unwrap())
        }
    };

    let file = if let Ok(file) = File::open(path).await {
        FramedRead::new(file, BytesCodec::new())
    } else {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("File not found".into())
            .unwrap());
    };

    Ok(Response::builder()
        .header("Content-Type", content_type)
        .body(Body::wrap_stream(file))
        .unwrap())
}
