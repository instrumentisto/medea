//! Client for a [WebDriver].
//!
//! [WebDriver]: https://w3.org/TR/webdriver

use std::sync::{mpsc, Arc};

use fantoccini::{Client, ClientBuilder, Locator};
use futures::lock::Mutex;
use serde::Deserialize;
use serde_json::{json, Value as Json};
use tokio::task;
use webdriver::{capabilities::Capabilities, common::WebWindow};

use super::{js::Statement, Error, Result};

/// Arguments for Chrome browser.
const CHROME_ARGS: &[&str] = &[
    "--use-fake-device-for-media-stream",
    "--use-fake-ui-for-media-stream",
    "--disable-web-security",
    "--disable-dev-shm-usage",
    "--no-sandbox",
];

/// Arguments for Firefox browser.
const FIREFOX_ARGS: &[&str] = &[];

/// Result returned from all the JS code executed in a browser.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum JsResult {
    /// [`Json`] value of a successful result.
    Ok(Json),

    /// [`Json`] value of an error result.
    Err(Json),
}

impl From<JsResult> for Result<Json> {
    #[inline]
    fn from(from: JsResult) -> Self {
        match from {
            JsResult::Ok(ok) => Self::Ok(ok),
            JsResult::Err(err) => Self::Err(Error::Js(err)),
        }
    }
}

/// Client for interacting with a browser through a [WebDriver] protocol.
///
/// [WebDriver]: https://w3.org/TR/webdriver
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct WebDriverClient {
    inner: Arc<Mutex<Inner>>,
    file_server_host: String,
}

impl WebDriverClient {
    /// Creates a new window in a browser and returns its ID.
    ///
    /// # Errors
    ///
    /// If failed to create or switch to a new [`WebWindow`].
    #[inline]
    pub async fn new_window(&self) -> Result<WebWindow> {
        self.inner
            .lock()
            .await
            .new_window(&self.file_server_host)
            .await
    }

    /// Switches to the provided [`WebWindow`] and executes the provided
    /// [`Statement`] in it.
    ///
    /// # Errors
    ///
    /// - If failed to switch to the provided [`WebWindow`].
    /// - If failed to execute JS statement.
    #[inline]
    pub async fn switch_to_window_and_execute(
        &self,
        window: WebWindow,
        exec: Statement,
    ) -> Result<Json> {
        self.inner
            .lock()
            .await
            .switch_to_window_and_execute(window, exec)
            .await
    }

    /// Synchronously closes a [WebDriver] session.
    ///
    /// [WebDriver]: https://w3.org/TR/webdriver
    ///
    /// # Panics
    ///
    /// If [`tokio::spawn()`] panics.
    pub fn blocking_close(&self) {
        let (tx, rx) = mpsc::channel();
        let client = self.inner.clone();
        tokio::spawn(async move {
            let mut inner = client.lock().await;
            inner.0.close().await.map_err(|e| dbg!("{:?}", e)).unwrap();
            tx.send(()).unwrap();
        });
        task::block_in_place(move || {
            rx.recv().unwrap();
        });
    }

    /// Synchronously closes the provided [`WebWindow`].
    ///
    /// # Panics
    ///
    /// If [`tokio::spawn()`] panics.
    pub fn blocking_window_close(&self, window: WebWindow) {
        let (tx, rx) = mpsc::channel();
        let client = self.inner.clone();
        tokio::spawn(async move {
            let mut client = client.lock().await;
            client.close_window(window).await;
            tx.send(()).unwrap();
        });
        task::block_in_place(move || {
            rx.recv().unwrap();
        });
    }
}

/// Builder for [`WebDriverClientBuilder`].
#[derive(Clone, Debug)]
pub struct WebDriverClientBuilder<'a> {
    webdriver_address: &'a str,
    file_server_host: String,
    headless_firefox: bool,
    headless_chrome: bool,
}

impl<'a> WebDriverClientBuilder<'a> {
    /// Creates new [`WebDriverClientBuilder`].
    #[must_use]
    pub fn new(webdriver_address: &'a str, file_server_host: &str) -> Self {
        Self {
            webdriver_address,
            file_server_host: file_server_host.to_owned(),
            headless_firefox: false,
            headless_chrome: false,
        }
    }

    /// Sets `moz:firefoxOptions` `--headless` for Firefox browser.
    #[must_use]
    pub fn headless_firefox(mut self, value: bool) -> Self {
        self.headless_firefox = value;
        self
    }

    /// Sets `goog:chromeOptions` `--headless` for Chrome browser.
    #[must_use]
    pub fn headless_chrome(mut self, value: bool) -> Self {
        self.headless_chrome = value;
        self
    }

    /// Creates a new [`WebDriverClient`] connected to a [WebDriver].
    ///
    /// # Errors
    ///
    /// If failed to locate [WebDriver] instance.
    ///
    /// [WebDriver]: https://w3.org/TR/webdriver
    pub async fn connect(
        self,
        file_server_host: &str,
    ) -> Result<WebDriverClient> {
        Ok(WebDriverClient {
            inner: Arc::new(Mutex::new(
                Inner::new(
                    self.webdriver_address,
                    self.headless_firefox,
                    self.headless_chrome,
                )
                .await?,
            )),
            file_server_host: file_server_host.to_owned(),
        })
    }
}

/// Inner implementation of a [`WebDriverClient`].
struct Inner(Client);

impl Inner {
    /// Creates a new [WebDriver] session.
    ///
    /// [WebDriver]: https://w3.org/TR/webdriver
    pub async fn new(
        webdriver_address: &str,
        headless_firefox: bool,
        headless_chrome: bool,
    ) -> Result<Self> {
        Ok(Self(
            ClientBuilder::native()
                .capabilities(Self::get_webdriver_capabilities(
                    headless_firefox,
                    headless_chrome,
                ))
                .connect(webdriver_address)
                .await?,
        ))
    }

    /// Executes the provided [`Statement`] in the current [`WebWindow`].
    ///
    /// # Errors
    ///
    /// - If JS exception was thrown while executing a JS code.
    /// - If failed to deserialize a result of the executed JS code.
    pub async fn execute(&mut self, statement: Statement) -> Result<Json> {
        let (inner_js, args) = statement.prepare();

        // language=JavaScript
        let js = format!(
            r#"
            (
                async () => {{
                    let callback = arguments[arguments.length - 1];
                    try {{
                        {executable_js}
                        callback({{ ok: lastResult }});
                    }} catch (e) {{
                        if (e.ptr != undefined) {{
                            callback({{
                                err: {{
                                    name: e.name(),
                                    message: e.message(),
                                    trace: e.trace(),
                                    source: e.source()
                                }}
                            }});
                        }} else {{
                            callback({{ err: e.toString() }});
                        }}
                    }}
                }}
            )();
            "#,
            executable_js = inner_js,
        );
        let res = self.0.execute_async(&js, args).await?;

        serde_json::from_value::<JsResult>(res)?.into()
    }

    /// Creates a new [`WebWindow`] and returns it's ID.
    ///
    /// Creates a `registry` in the created [`WebWindow`].
    ///
    /// # Errors
    ///
    /// - If failed to create new [`WebWindow`].
    /// - If `index.html` wasn't found at `file_server_host`.
    pub async fn new_window(
        &mut self,
        file_server_host: &str,
    ) -> Result<WebWindow> {
        let window = WebWindow(self.0.new_window(true).await?.handle);
        self.0.switch_to_window(window.clone()).await?;
        self.0
            .goto(&format!("http://{}/index.html", file_server_host))
            .await?;
        self.0.wait_for_find(Locator::Id("loaded")).await?;

        self.execute(Statement::new(
            // language=JavaScript
            r#"
                async () => {
                    window.registry = new Map();
                }
            "#,
            vec![],
        ))
        .await?;

        Ok(window)
    }

    /// Switches to the provided [`WebWindow`] and executes the provided
    /// [`Statement`].
    pub async fn switch_to_window_and_execute(
        &mut self,
        window: WebWindow,
        exec: Statement,
    ) -> Result<Json> {
        self.0.switch_to_window(window).await?;
        self.execute(exec).await
    }

    /// Closes the provided [`WebWindow`].
    pub async fn close_window(&mut self, window: WebWindow) {
        if self.0.switch_to_window(window).await.is_ok() {
            drop(self.0.close_window().await);
        }
    }

    /// Returns `moz:firefoxOptions` for a Firefox browser.
    fn get_firefox_caps(value: bool) -> serde_json::Value {
        let mut args = FIREFOX_ARGS.to_vec();
        if value {
            args.push("--headless");
        }
        json!({
            "prefs": {
                "media.navigator.streams.fake": true,
                "media.navigator.permission.disabled": true,
                "media.autoplay.enabled": true,
                "media.autoplay.enabled.user-gestures-needed ": false,
                "media.autoplay.ask-permission": false,
                "media.autoplay.default": 0,
            },
            "args": args,
        })
    }

    /// Returns `goog:chromeOptions` for a Chrome browser.
    fn get_chrome_caps(value: bool) -> serde_json::Value {
        let mut args = CHROME_ARGS.to_vec();
        if value {
            args.push("--headless");
        }
        json!({ "args": args })
    }

    /// Returns [WebDriver capabilities][1] for Chrome and Firefox browsers.
    ///
    /// [1]: https:/mdn.io/Web/WebDriver/Capabilities
    fn get_webdriver_capabilities(
        headless_firefox: bool,
        headless_chrome: bool,
    ) -> Capabilities {
        let mut caps = Capabilities::new();
        caps.insert(
            "moz:firefoxOptions".to_owned(),
            Self::get_firefox_caps(headless_firefox),
        );
        caps.insert(
            "goog:chromeOptions".to_owned(),
            Self::get_chrome_caps(headless_chrome),
        );
        caps
    }
}
