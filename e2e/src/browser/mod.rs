//! Implementation of the object and methods for interracting with browser
//! through [WebDriver] protocol.
//!
//! [WebDriver]: https://www.w3.org/TR/webdriver/

mod client;
mod executable;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use derive_more::From;
use serde_json::Value as Json;
use webdriver::common::WebWindow;

use self::client::WebClient;

pub use self::executable::JsExecutable;

#[derive(Debug, From)]
pub enum Error {
    #[from(ignore)]
    Js(Json),
    WebDriverCmd(fantoccini::error::CmdError),
    WebDriverSession(fantoccini::error::NewSessionError),
}

type Result<T> = std::result::Result<T, Error>;

pub struct WindowWebClient {
    client: WebClient,
    window: WebWindow,
    rc: Arc<AtomicUsize>,
}

impl Clone for WindowWebClient {
    fn clone(&self) -> Self {
        self.rc.fetch_add(1, Ordering::SeqCst);
        Self {
            client: self.client.clone(),
            window: self.window.clone(),
            rc: Arc::clone(&self.rc),
        }
    }
}

impl Drop for WindowWebClient {
    fn drop(&mut self) {
        if self.rc.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.client.blocking_window_close(self.window.clone());
        }
    }
}

impl WindowWebClient {
    async fn new(mut client: WebClient) -> Self {
        let window = client.new_window().await;

        Self {
            client,
            window,
            rc: Arc::new(AtomicUsize::new(1)),
        }
    }

    pub async fn execute(&mut self, exec: JsExecutable) -> Result<Json> {
        self.client
            .switch_to_window_and_execute(self.window.clone(), exec)
            .await
    }
}

pub struct RootWebClient(WebClient);

impl RootWebClient {
    pub async fn new() -> Self {
        Self(WebClient::new().await.unwrap())
    }

    pub async fn new_window(&mut self) -> WindowWebClient {
        WindowWebClient::new(self.0.clone()).await
    }
}

impl Drop for RootWebClient {
    fn drop(&mut self) {
        self.0.blocking_close();
    }
}
