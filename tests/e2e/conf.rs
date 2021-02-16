//! All configurable properties of the E2E tests runner.

use std::env;

use once_cell::sync::Lazy;

/// Generates static config variable which will be lazily obtained from the
/// environment variables and if failed, default one will be used.
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
    /// Address of the [WebDriver] client.
    ///
    /// Default: `http://127.0.0.1:4444`.
    ///
    /// [WebDriver]: https://www.w3.org/TR/webdriver/
    WEBDRIVER_ADDR
        || "http://127.0.0.1:4444"
);

env_var!(
    /// Address of the Control API mock server.
    ///
    /// Default: `http://127.0.0.1:8000/control-api`.
    CONTROL_API_ADDR
        || "http://127.0.0.1:8000/control-api"
);

env_var!(
    /// Address for the Client API.
    ///
    /// Default: `ws://127.0.0.1:8080/ws`.
    CLIENT_API_ADDR
        || "ws://127.0.0.1:8080/ws"
);

env_var!(
    /// Address where [`FileServer`] will be hosted.
    ///
    /// Default: `ws://127.0.0.1:8080/ws`.
    ///
    /// [`FileServer`]: crate::file_server::FileServer
    FILE_SERVER_ADDR
        || "127.0.0.1:30000"
);

env_var!(
    /// Path to the directory where compiled `jason` is stored.
    ///
    /// Default: `jason/pkg`.
    JASON_DIR_PATH
        || "jason/pkg"
);

env_var!(
    /// Path to the `index.html` for all tests.
    INDEX_PATH
        || "tests/e2e/index.html"
);

env_var!(
    /// Path to the Cucumber Features which are should be ran.
    FEATURES_PATH
        || "tests/e2e/features"
);

/// Flag which indicates that tests should be ran in the headless browser.
///
/// Default: `true`.
pub static HEADLESS: Lazy<bool> = Lazy::new(|| {
    env::var("HEADLESS").map_or(true, |v| v.to_ascii_lowercase() == "true")
});
