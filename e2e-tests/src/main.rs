pub mod mocha_result;
pub mod test_runner;

use std::{fs::canonicalize, path::PathBuf};

use actix::System;
use actix_files::NamedFile;
use actix_web::{
    dev::Server, web, App, HttpRequest, HttpServer, Result as HttpResult,
};
use futures::Future;

pub const TESTS_ADDR: &str = "127.0.0.1:8088";

fn index(req: HttpRequest) -> HttpResult<NamedFile> {
    let path: PathBuf = req.match_info().query("filename").parse().unwrap();
    Ok(NamedFile::open(path)?)
}

fn run_http_server() -> Server {
    HttpServer::new(|| App::new().route("{filename:.*}", web::get().to(index)))
        .bind(TESTS_ADDR)
        .unwrap()
        .start()
}

fn get_path_to_tests_from_args() -> PathBuf {
    let path_to_tests = std::env::args().skip(1).next().unwrap();
    let path_to_tests = PathBuf::from(path_to_tests);
    let path_to_tests = canonicalize(path_to_tests).unwrap();
    path_to_tests
}

// TODO: make optional headless
fn main() {
    actix::run(|| {
        let server = run_http_server();
        let path_to_tests = get_path_to_tests_from_args();
        test_runner::run(path_to_tests)
            .and_then(move |_| server.stop(true))
            .map(|_| System::current().stop())
    })
    .unwrap();
}
