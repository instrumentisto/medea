mod firewall;
mod gremlin;
mod prelude;
mod server;

use crate::{firewall::Firewall, gremlin::Gremlin};
use actix::Actor;
use slog::{o, Drain};

fn main() {
    dotenv::dotenv().ok();

    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_envlogger::new(drain).fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let logger = slog::Logger::root(drain, o!());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();

    let sys = actix::System::new("control-api-mock");
    let firewall = Firewall::new().unwrap();
    let gremlin = Gremlin::new(firewall.clone()).start();
    server::run(firewall, gremlin);
    sys.run().unwrap();
}
