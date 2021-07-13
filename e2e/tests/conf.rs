//! All configurable properties of E2E tests runner.

use std::env;

use once_cell::sync::Lazy;

/// Generates static config variable which will be lazily obtained from the
/// environment variables falling back to a default one.
macro_rules! env_var {
    (
        $(#[$meta:meta])*
        $name:ident || $default:expr
    ) => {
        $(#[$meta])*
        pub static $name: Lazy<String> = Lazy::new(|| {
            env::var(stringify!($name))
                .unwrap_or_else(|_| $default.to_string())
        });
    };
}

env_var!(
    /// Address of a [WebDriver] client.
    ///
    /// Default: `http://127.0.0.1:4444`
    ///
    /// [WebDriver]: https://w3.org/TR/webdriver
    WEBDRIVER_ADDR
        || "http://127.0.0.1:4444"
);

env_var!(
    /// Address of a Control API mock server.
    ///
    /// Default: `http://127.0.0.1:8000`
    CONTROL_API_ADDR
        || "http://127.0.0.1:8000"
);

env_var!(
    /// Address a Client API WebSocket endpoint.
    ///
    /// Default: `ws://127.0.0.1:8001/ws`
    CLIENT_API_ADDR
        || "ws://127.0.0.1:8001/ws"
);

env_var!(
    /// Host of a [`FileServer`].
    ///
    /// Default: `127.0.0.1:30000`
    ///
    /// [`FileServer`]: crate::file_server::FileServer
    FILE_SERVER_HOST
        || "127.0.0.1:30000"
);

env_var!(
    /// Path to a Cucumber features which should be run.
    FEATURES_PATH
        || "tests/features"
);

/// Indicator whether tests should run in a headless browser's mode.
///
/// Default: `true`
pub static HEADLESS: Lazy<bool> = Lazy::new(|| {
    env::var("HEADLESS").map_or(true, |v| v.to_ascii_lowercase() == "true")
});
