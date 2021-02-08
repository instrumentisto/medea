use std::env;

use once_cell::sync::Lazy;

/// Address of the [WebDriver] client.
///
/// Default: `http://127.0.0.1:4444`.
///
/// [WebDriver]: https://www.w3.org/TR/webdriver/
pub static WEBDRIVER_ADDR: Lazy<String> = Lazy::new(|| {
    env::var("WEBDRIVER_ADDR")
        .unwrap_or_else(|_| "http://127.0.0.1:4444".to_string())
});

/// Address of the Control API mock server.
///
/// Default: `http://127.0.0.1:8000/control-api`.
pub static CONTROL_API_ADDR: Lazy<String> = Lazy::new(|| {
    env::var("CONTROL_API_ADDR")
        .unwrap_or_else(|_| "http://127.0.0.1:8000/control-api".to_string())
});

/// Address for the Client API.
///
/// Default: `ws://127.0.0.1:8080/ws`.
pub static CLIENT_API_ADDR: Lazy<String> = Lazy::new(|| {
    env::var("CLIENT_API_ADDR")
        .unwrap_or_else(|_| "ws://127.0.0.1:8080/ws".to_string())
});

/// Address where [`FileServer`] will be hosted.
///
/// Default: `ws://127.0.0.1:8080/ws`.
///
/// [`FileServer`]: crate::file_server::FileServer
pub static FILE_SERVER_ADDR: Lazy<String> = Lazy::new(|| {
    env::var("FILE_SERVER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:30000".to_string())
});

/// Flag which indicates that tests should be ran in the headless browser.
///
/// Default: `true`.
pub static HEADLESS: Lazy<bool> = Lazy::new(|| {
    env::var("HEADLESS").map_or(true, |v| v.to_ascii_lowercase() == "true")
});

/// Path to the directory where compiled `jason` is stored.
pub static JASON_DIR_PATH: Lazy<String> = Lazy::new(|| {
    env::var("JASON_DIR_PATH").unwrap_or_else(|_| "jason/pkg".to_string())
});

/// Path to the `index.html` for all tests.
pub static INDEX_PATH: Lazy<String> = Lazy::new(|| {
    env::var("INDEX_PATH").unwrap_or_else(|_| "e2e/index.html".to_string())
});

/// Path to the Cucumber Features which are should be ran.
pub static FEATURES_PATH: Lazy<String> = Lazy::new(|| {
    env::var("FEATURES_PATH").unwrap_or_else(|_| "e2e/features".to_string())
});
