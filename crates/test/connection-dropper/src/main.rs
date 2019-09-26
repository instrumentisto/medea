//! Server which provides API for opening and closing some port.
//!
//! Used for connection loss E2E tests of [Medea]/[Jason].
//!
//! [Medea]: https://github.com/instrumentisto/medea
//! [Jason]: https://github.com/instrumentisto/medea/tree/master/jason

mod firewall;
mod gremlin;
mod prelude;
mod server;

use clap::{
    app_from_crate, crate_authors, crate_description, crate_name,
    crate_version, Arg,
};
use slog::{o, Drain};

#[link(name = "c")]
extern "C" {
    fn geteuid() -> u32;
}

fn main() {
    // We need root permission because we use 'iptables'.
    unsafe {
        if geteuid() != 0 {
            panic!("You cannot run connection-dropper unless you are root.");
        }
    }

    let opts = app_from_crate!()
        .arg(
            Arg::with_name("addr")
                .help("Address where dropper control server will be hosted.")
                .default_value("127.0.0.1:8500")
                .long("addr")
                .short("a"),
        )
        .arg(
            Arg::with_name("port")
                .help("Port which you want to open/close.")
                .default_value("8090")
                .long("port")
                .short("p"),
        )
        .arg(
            Arg::with_name("gremlin-max-wait")
                .help("Minimum port's loss/availability time.")
                .default_value("5")
                .long("min-wait")
                .short("m"),
        )
        .arg(
            Arg::with_name("gremlin-min-wait")
                .help("Maximum port's loss/availability time.")
                .default_value("15")
                .long("max-wait")
                .short("x"),
        )
        .get_matches();

    dotenv::dotenv().ok();

    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_envlogger::new(drain).fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let logger = slog::Logger::root(drain, o!());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();

    let sys = actix::System::new("control-api-mock");
    server::run(opts);
    sys.run().unwrap();
}
