//! REST mock server for gRPC [Medea]'s [Control API].
//!
//! [Medea]: https://github.com/instrumentisto/medea
//! [Control API]: https://tinyurl.com/yxsqplq7

use clap::{
    app_from_crate, crate_authors, crate_description, crate_name,
    crate_version, Arg,
};
use medea_control_api_mock::{api, callback, init_logger};

fn main() {
    dotenv::dotenv().ok();

    let opts = app_from_crate!()
        .arg(
            Arg::with_name("addr")
                .help("Address to host medea-control-api-mock-server on.")
                .default_value("0.0.0.0:8000")
                .long("addr")
                .short("a"),
        )
        .arg(
            Arg::with_name("medea_addr")
                .help("Address to Medea's gRPC control API.")
                .default_value("http://0.0.0.0:6565")
                .long("medea-addr")
                .short("m"),
        )
        .arg(
            Arg::with_name("callback_port")
                .help("Port to listen by gRPC Control API Callback service.")
                .default_value("9099")
                .long("callback-port")
                .short("p"),
        )
        .arg(
            Arg::with_name("callback_host")
                .help("Address to host gRPC Control API Callback service on.")
                .default_value("0.0.0.0")
                .long("callback-host")
                .short("c"),
        )
        .get_matches();

    let _log_guard = init_logger();

    actix_web::rt::System::new().block_on(async move {
        let callback_server = callback::server::run(&opts).await;
        api::run(&opts, callback_server).await;
    });
}
