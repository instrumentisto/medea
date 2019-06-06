//! Utils useful for e2e testing of medea.

use std::{
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering},
    sync::{Arc, Mutex},
};

use actix::System;
use medea::{
    api::client::server, conf::Conf, conf::Server,
    signalling::room_repo::RoomsRepository, start_static_rooms,
};

/// Test ports counter. Used for dealing with async testing.
/// Enumerating start from 49152 because
/// based on [Registered by IANA ports][1] this is the last reserved port.
/// 16384 ought to be enough for anybody.
///
/// Use `get_port_for_test()` instead of accessing this var directly.
///
/// [1]: https://en.wikipedia.org/wiki/List_of_TCP_and_UDP_port_numbers
static LAST_TEST_PORT: AtomicUsize = AtomicUsize::new(49152);

/// Use it for getting port for testing.
pub fn get_port_for_test() -> u16 {
    LAST_TEST_PORT.fetch_add(1, Ordering::Relaxed) as u16
}

/// Run medea server. This function lock main thread until server is up.
/// Server starts in different thread and `join`'ed with main thread.
/// When test is done, server will be destroyed.
///
/// Server load all specs from `tests/specs`.
///
/// Provide `test_name` same as your test function's name. This will
/// help you when server panic in some test case.
pub fn run_test_server(test_name: &'static str) -> u16 {
    let bind_port = get_port_for_test();

    let is_server_starting = Arc::new(Mutex::new(Cell::new(true)));
    let is_server_starting_ref = Arc::clone(&is_server_starting);
    let builder = std::thread::Builder::new().name(test_name.to_string());

    let server_thread = builder
        .spawn(move || {
            let _ = System::new(format!("test-medea-server-{}", test_name));

            let config = Conf {
                server: Server {
                    static_specs_path: Some("tests/specs".to_string()),
                    bind_port,
                    ..Default::default()
                },
                ..Default::default()
            };

            match start_static_rooms(&config) {
                Ok(r) => {
                    let room_repo = RoomsRepository::new(r);
                    server::run(room_repo, config);
                }
                Err(e) => {
                    panic!("Server not started because of error: '{}'", e);
                }
            };
            let is_server_starting_guard =
                is_server_starting_ref.lock().unwrap();
            is_server_starting_guard.set(false);
        })
        .unwrap();

    // Wait until server is up
    while is_server_starting.lock().unwrap().get() {}

    server_thread.join().unwrap();

    bind_port
}
