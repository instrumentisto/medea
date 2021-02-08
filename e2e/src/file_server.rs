//! Implementation of the HTTP file server which will share files required for
//! tests.

use std::path::PathBuf;

use futures::{channel::oneshot, future::select};
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, Server, StatusCode,
};
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};

use crate::conf;

const NOT_FOUND: &[u8] = b"Not found";

/// File server which will share `index.html` and compiled `jason` library.
pub struct FileServer(Option<oneshot::Sender<()>>);

impl FileServer {
    /// Starts file server which will share `index.html` and compiled `jason`
    /// library.
    pub fn run() -> Self {
        let make_service = make_service_fn(|_| async {
            Ok::<_, hyper::Error>(service_fn(response_files))
        });
        let server = Server::bind(&conf::FILE_SERVER_ADDR.parse().unwrap())
            .serve(make_service);

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

/// [`Response`] for the files which are not found on the server.
fn not_found() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(NOT_FOUND.into())
        .unwrap()
}

/// Handles all files requests to this [`FileServer`].
///
/// Returns [`not_found`] if path is not for `jason` directory or `index.html`
/// file.
async fn response_files(
    req: Request<Body>,
) -> Result<Response<Body>, hyper::Error> {
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
        _ => unreachable!(
            "File Server shares only index.html and jason directory."
        ),
    };

    let mime = match path.extension().unwrap().to_str().unwrap() {
        "js" => "text/javascript",
        "html" => "text/html",
        "wasm" => "application/wasm",
        _ => unreachable!(
            "Only JS, HTML and WASM files should be shared by this server."
        ),
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
