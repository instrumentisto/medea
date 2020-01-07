//! E2E tests runner.

// TODO: Remove `clippy::must_use_candidate` once the issue below is resolved:
//       https://github.com/rust-lang/rust-clippy/issues/4779
#![allow(clippy::module_name_repetitions, clippy::must_use_candidate)]

pub mod mocha_result;
pub mod test_runner;

use std::path::PathBuf;

use actix_files::NamedFile;
use actix_web::{
    dev::Server, web, App, HttpRequest, HttpServer, Result as HttpResult,
};
use clap::{
    app_from_crate, crate_authors, crate_description, crate_name,
    crate_version, Arg, ArgMatches,
};

use crate::test_runner::TestRunnerBuilder;

/// HTTP endpoint which return requested file from this dir.
/// Used for loading tests.
#[allow(clippy::needless_pass_by_value)]
async fn index(req: HttpRequest) -> HttpResult<NamedFile> {
    let path: PathBuf = req.match_info().query("filename").parse().unwrap();
    Ok(NamedFile::open(path)?)
}

/// Start server which serve files from this dir.
///
/// WebDriver's browser will go into test files from this server.
///
/// This is needed because restriction for type=module scripts.
fn run_test_files_server(addr: &str) -> Server {
    HttpServer::new(|| {
        App::new()
            .service(web::resource("{filename:.*}").route(web::get().to(index)))
    })
    .bind(addr)
    .unwrap()
    .run()
}

/// Returns [`PathBuf`] to e2e-tests path.
pub fn get_path_to_tests() -> PathBuf {
    let mut test_path = std::env::current_dir().unwrap();
    test_path.push("e2e-tests");
    test_path
}

/// Returns [`PathBuf`] to test/test dir from clap's [`ArgMatches`].
fn get_path_to_tests_from_args(opts: &ArgMatches) -> PathBuf {
    let mut test_path = get_path_to_tests();
    if let Some(path_to_test) = opts.value_of("spec_path") {
        test_path.push(path_to_test);
        if !test_path.exists() {
            panic!("Test '{}' doesn't exist!", path_to_test);
        }
    }
    test_path
}

fn get_opts<'a>() -> ArgMatches<'a> {
    app_from_crate!()
        .arg(
            Arg::with_name("headless")
                .help("Run tests in headless browser.")
                .long("headless"),
        )
        .arg(
            Arg::with_name("spec_path")
                .help("Run only specified spec.")
                .index(1),
        )
        .arg(
            Arg::with_name("tests_files_addr")
                .help("Address where html test files will be hosted.")
                .default_value("127.0.0.1:9000")
                .long("files-host")
                .short("f"),
        )
        .arg(
            Arg::with_name("webdriver_addr")
                .help("Address to running webdriver.")
                .default_value("http://127.0.0.1:4444")
                .long("webdriver-addr")
                .short("w"),
        )
        .arg(
            Arg::with_name("wait_on_fail")
                .help(
                    "If tests fails then runner will don't close browser \
                     until you press <Enter>.",
                )
                .long("wait-on-fail"),
        )
        .get_matches()
}

#[actix_rt::main]
async fn main() {
    let opts = get_opts();

    let server =
        run_test_files_server(opts.value_of("tests_files_addr").unwrap());

    let path_to_tests = get_path_to_tests_from_args(&opts);
    TestRunnerBuilder::default()
        .test_addr(opts.value_of("tests_files_addr").unwrap())
        .is_wait_on_fail_mode(opts.is_present("wait_on_fail"))
        .webdriver_addr(opts.value_of("webdriver_addr").unwrap())
        .is_headless(opts.is_present("headless"))
        .build()
        .unwrap()
        .run(path_to_tests)
        .await
        .unwrap();

    server.stop(true).await;
}
