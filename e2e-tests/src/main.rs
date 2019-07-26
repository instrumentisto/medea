//! E2e tests runner.

pub mod mocha_result;
pub mod test_runner;

use std::{fs::canonicalize, path::PathBuf};

use actix::System;
use actix_files::NamedFile;
use actix_web::{
    dev::Server, web, App, HttpRequest, HttpServer, Result as HttpResult,
};
use clap::{App as ClapApp, Arg, ArgMatches};
use futures::Future;

use crate::test_runner::TestRunner;

/// HTTP endpoint which return requested file from this dir.
/// Used for loading tests.
#[allow(clippy::needless_pass_by_value)]
fn index(req: HttpRequest) -> HttpResult<NamedFile> {
    let path: PathBuf = req.match_info().query("filename").parse().unwrap();
    Ok(NamedFile::open(path)?)
}

/// Start server which serve files from this dir.
///
/// WebDriver's browser will go into test files from this server.
///
/// This is needed because restriction for type=module scripts.
fn run_test_files_server(addr: &str) -> Server {
    HttpServer::new(|| App::new().route("{filename:.*}", web::get().to(index)))
        .bind(addr)
        .unwrap()
        .start()
}

/// Returns [`PathBuf`] to test/test dir from clap's [`ArgMatches`].
fn get_path_to_tests_from_args(opts: &ArgMatches) -> PathBuf {
    let path_to_tests = opts.value_of("specs_path").unwrap();
    let path_to_tests = PathBuf::from(path_to_tests);
    canonicalize(path_to_tests).unwrap()
}

fn main() {
    let opts = ClapApp::new("e2e-tests-runner")
        .arg(
            Arg::with_name("headless")
                .help("Run tests in headless browser")
                .long("headless"),
        )
        .arg(
            Arg::with_name("specs_path")
                .help("Path to specs/spec")
                .index(1)
                .required(true),
        )
        .arg(
            Arg::with_name("tests_files_addr")
                .help("Address for tests files host server")
                .default_value("localhost:9000")
                .long("files-host")
                .short("f"),
        )
        .arg(
            Arg::with_name("webdriver_addr")
                .help("Address to running webdriver")
                .default_value("http://localhost:4444")
                .long("webdriver-addr")
                .short("w"),
        )
        .get_matches();

    actix::run(|| {
        let server =
            run_test_files_server(opts.value_of("tests_files_addr").unwrap());
        let path_to_tests = get_path_to_tests_from_args(&opts);
        TestRunner::run(path_to_tests, &opts)
            .and_then(move |_| server.stop(true))
            .map(|_| System::current().stop())
    })
    .unwrap();
}
