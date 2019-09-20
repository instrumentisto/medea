mod firewall;
mod gremlin;
mod prelude;
mod server;

use actix::Actor;
use slog::{o, Drain};

use crate::{firewall::Firewall, gremlin::Gremlin};

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
