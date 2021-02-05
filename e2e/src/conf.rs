use once_cell::sync::Lazy;
use std::env;

pub static WEBDRIVER_ADDR: Lazy<String> = Lazy::new(|| {
    env::var("WEBDRIVER_ADDR")
        .unwrap_or_else(|_| "http://127.0.0.1:4444".to_string())
});

pub static CONTROL_API_ADDR: Lazy<String> = Lazy::new(|| {
    env::var("CONTROL_API_ADDR")
        .unwrap_or_else(|_| "http://127.0.0.1:8000/control-api".to_string())
});

pub static CLIENT_API_ADDR: Lazy<String> = Lazy::new(|| {
    env::var("CLIENT_API_ADDR")
        .unwrap_or_else(|_| "ws://127.0.0.1:8080/ws".to_string())
});

pub static FILE_SERVER_ADDR: Lazy<String> = Lazy::new(|| {
    env::var("FILE_SERVER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:30000".to_string())
});

pub static HEADLESS: Lazy<bool> = Lazy::new(|| {
    env::var("HEADLESS").map_or(true, |v| v.to_ascii_lowercase() == "true")
});

pub static JASON_DIR_PATH: Lazy<String> = Lazy::new(|| {
    env::var("JASON_DIR_PATH").unwrap_or_else(|_| "jason/pkg".to_string())
});

pub static INDEX_PATH: Lazy<String> = Lazy::new(|| {
    env::var("INDEX_PATH").unwrap_or_else(|_| "e2e/index.html".to_string())
});

pub static FEATURES_PATH: Lazy<String> = Lazy::new(|| {
    env::var("FEATURES_PATH").unwrap_or_else(|_| "e2e/features".to_string())
});
