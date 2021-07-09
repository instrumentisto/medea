//! Interaction with browser through a [WebDriver] protocol.
//!
//! [WebDriver]: https://w3.org/TR/webdriver

mod client;
mod js;
pub mod mock;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use derive_more::{Display, Error, From};
use serde_json::Value as Json;
use webdriver::common::WebWindow;

pub use self::client::{WebDriverClient, WebDriverClientBuilder};

#[doc(inline)]
pub use self::js::Statement;

/// All errors which can happen while working with a browser.
#[derive(Debug, Display, Error, From)]
pub enum Error {
    /// JS exception was thrown while executing a JS code.
    #[from(ignore)]
    Js(#[error(not(source))] Json),

    /// Error occurred while executing some browser action by a [WebDriver].
    ///
    /// [WebDriver]: https://w3.org/TR/webdriver
    WebDriverCmd(fantoccini::error::CmdError),

    /// Error occurred while attempting to establish a [WebDriver] session.
    ///
    /// [WebDriver]: https://w3.org/TR/webdriver
    WebDriverSession(fantoccini::error::NewSessionError),

    /// Failed to deserialize a result of the executed JS code.
    ///
    /// Should never happen.
    Deserialize(serde_json::Error),
}

/// Shortcut for a [`Result`] with an [`Error`](enum@Error) inside.
///
/// [`Result`]: std::result::Result
type Result<T> = std::result::Result<T, Error>;

/// [WebDriver] handle of a browser window.
///
/// All JS code executed by [`Window::execute()`] will run in the right browser
/// window.
///
/// Window is closed once all [`WebWindow`]s for this window are [`Drop`]ped.
///
/// [WebDriver]: https://w3.org/TR/webdriver
pub struct Window {
    /// Client for interacting with a browser through [WebDriver].
    ///
    /// [WebDriver]: https://w3.org/TR/webdriver
    client: WebDriverClient,

    /// ID of the window in which this [`Window`] should execute everything.
    window: WebWindow,

    /// Count of this [`Window`] references.
    ///
    /// Used in a [`Drop`] implementation of this [`Window`].
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
    /// Creates a new [`Window`] in the provided [`WebDriverClient`].
    async fn new(client: WebDriverClient) -> Self {
        let window = client.new_window().await.unwrap();

        let this = Self {
            client,
            window,
            rc: Arc::new(AtomicUsize::new(1)),
        };
        mock::instantiate_mocks(&this).await;
        this
    }

    /// Executes the provided [`Statement`] in this [`Window`].
    ///
    /// # Errors
    ///
    /// - If failed to switch to the provided [`WebWindow`].
    /// - If failed to execute JS statement.
    #[inline]
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
/// [WebDriver] session will be closed on this object's [`Drop`].
///
/// [WebDriver]: https://w3.org/TR/webdriver
#[derive(From)]
pub struct WindowFactory(WebDriverClient);

impl WindowFactory {
    /// Returns a new [`WindowFactory`] from [`WebDriverClient`].
    #[inline]
    pub async fn new(client: WebDriverClient) -> Self {
        Self(client)
    }

    /// Creates and returns a new [`Window`].
    #[inline]
    pub async fn new_window(&self) -> Window {
        Window::new(self.0.clone()).await
    }
}

impl Drop for WindowFactory {
    #[inline]
    fn drop(&mut self) {
        self.0.blocking_close();
    }
}
