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

use derive_more::{Display, Error, From};
use serde_json::Value as Json;
use webdriver::common::WebWindow;

use self::client::WebClient;

pub use self::executable::JsExecutable;

/// All errors which can happen while working with browser.
#[derive(Debug, Display, Error, From)]
pub enum Error {
    /// JS exception was thrown while executing JS code.
    #[from(ignore)]
    Js(#[error(not(source))] Json),

    /// An error occurred while executing some browser action by [WebDriver].
    ///
    /// [WebDriver]: https://www.w3.org/TR/webdriver/
    WebDriverCmd(fantoccini::error::CmdError),

    /// An error occured while attempting to establish a [WebDriver] session.
    ///
    /// [WebDriver]: https://www.w3.org/TR/webdriver/
    WebDriverSession(fantoccini::error::NewSessionError),

    /// Failed to deserialize result from the executed JS code.
    ///
    /// Should never happen.
    ResultDeserialize(serde_json::Error),
}

type Result<T> = std::result::Result<T, Error>;

/// [WebDriver] for some concrete browser window.
///
/// All JS executed by [`WindowWebClient::execute`] will be ran in the right
/// browser window.
///
/// Window will be closed when all [`WindowWebClient`] for this window will be
/// [`Drop`]ped.
///
/// [WebDriver]: https://www.w3.org/TR/webdriver/
pub struct WindowWebClient {
    /// Client for interacting with browser through [WebDriver].
    ///
    /// [WebDriver]: https://www.w3.org/TR/webdriver/
    client: WebClient,

    /// ID of window in which this [`WindowWebClient`] should execute
    /// everything.
    window: WebWindow,

    /// Count of [`WindowWebClient`] references.
    ///
    /// Used in the [`Drop`] implementation of the [`WindowWebClient`].
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
    /// Creates new window in the provided [`WebClient`] and returns
    /// [`WindowWebClient`] for the created window.
    async fn new(client: WebClient) -> Self {
        let window = client.new_window().await.unwrap();

        Self {
            client,
            window,
            rc: Arc::new(AtomicUsize::new(1)),
        }
    }

    /// Executes provided [`JsExecutable`] in window which this
    /// [`WindowWebClient`] represents.
    pub async fn execute(&self, exec: JsExecutable) -> Result<Json> {
        self.client
            .switch_to_window_and_execute(self.window.clone(), exec)
            .await
    }
}

/// Root [WebDriver] client for some browser.
///
/// This client can create new [`WindowWebClient`]s.
///
/// [WebDriver] session will be closed on this object [`Drop::drop`].
///
/// [WebDriver]: https://www.w3.org/TR/webdriver/
pub struct RootWebClient(WebClient);

impl RootWebClient {
    /// Returns new [`RootWebClient`].
    pub async fn new() -> Result<Self> {
        Ok(Self(WebClient::new().await?))
    }

    /// Creates and returns new [`WindowWebClient`].
    pub async fn new_window(&self) -> WindowWebClient {
        WindowWebClient::new(self.0.clone()).await
    }
}

impl Drop for RootWebClient {
    fn drop(&mut self) {
        self.0.blocking_close();
    }
}
