//! Interaction with browser through a [WebDriver] protocol.
//!
//! [WebDriver]: https://w3.org/TR/webdriver

mod client;
mod js;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use derive_more::{Display, Error, From};
use serde_json::Value as Json;
use webdriver::common::WebWindow;

use self::client::WebDriverClient;

#[doc(inline)]
pub use self::js::Statement;

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
    Deserialize(serde_json::Error),
}

type Result<T> = std::result::Result<T, Error>;

/// [WebDriver] for some concrete browser window.
///
/// All JS executed by [`Window::execute`] will be ran in the right
/// browser window.
///
/// Window will be closed when all [`Window`] for this window will be
/// [`Drop`]ped.
///
/// [WebDriver]: https://www.w3.org/TR/webdriver/
pub struct Window {
    /// Client for interacting with browser through [WebDriver].
    ///
    /// [WebDriver]: https://www.w3.org/TR/webdriver/
    client: WebDriverClient,

    /// ID of window in which this [`Window`] should execute
    /// everything.
    window: WebWindow,

    /// Count of [`Window`] references.
    ///
    /// Used in the [`Drop`] implementation of the [`Window`].
    rc: Arc<AtomicUsize>,
}

impl Clone for Window {
    fn clone(&self) -> Self {
        self.rc.fetch_add(1, Ordering::SeqCst);
        Self {
            client: self.client.clone(),
            window: self.window.clone(),
            rc: Arc::clone(&self.rc),
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if self.rc.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.client.blocking_window_close(self.window.clone());
        }
    }
}

impl Window {
    /// Creates new window in the provided [`WebDriverClient`] and returns
    /// [`Window`] for the created window.
    async fn new(client: WebDriverClient) -> Self {
        let window = client.new_window().await.unwrap();

        Self {
            client,
            window,
            rc: Arc::new(AtomicUsize::new(1)),
        }
    }

    /// Executes provided [`Statement`] in window which this [`Window`]
    /// represents.
    pub async fn execute(&self, exec: Statement) -> Result<Json> {
        self.client
            .switch_to_window_and_execute(self.window.clone(), exec)
            .await
    }
}

/// Root [WebDriver] client for some browser.
///
/// This client can create new [`Window`]s.
///
/// [WebDriver] session will be closed on this object [`Drop::drop`].
///
/// [WebDriver]: https://www.w3.org/TR/webdriver/
pub struct WindowFactory(WebDriverClient);

impl WindowFactory {
    /// Returns new [`WindowFactory`].
    pub async fn new() -> Result<Self> {
        Ok(Self(WebDriverClient::new().await?))
    }

    /// Creates and returns new [`Window`].
    pub async fn new_window(&self) -> Window {
        Window::new(self.0.clone()).await
    }
}

impl Drop for WindowFactory {
    fn drop(&mut self) {
        self.0.blocking_close();
    }
}
