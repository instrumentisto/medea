//! Client for a [WebDriver].
//!
//! [WebDriver]: https://w3.org/TR/webdriver

use std::sync::{mpsc, Arc};

use fantoccini::{Client, ClientBuilder, Locator};
use futures::lock::Mutex;
use serde::Deserialize;
use serde_json::{json, Value as Json};
use tokio_1::{self as tokio, task};
use webdriver::{capabilities::Capabilities, common::WebWindow};

use crate::conf;

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
#[derive(Clone, Debug)]
pub struct WebDriverClient(Arc<Mutex<Inner>>);

impl WebDriverClient {
    /// Creates a new [`WebDriverClient`] connected to a [WebDriver].
    ///
    /// [WebDriver]: https://w3.org/TR/webdriver
    #[inline]
    pub async fn new() -> Result<Self> {
        Ok(Self(Arc::new(Mutex::new(Inner::new().await?))))
    }

    /// Creates a new window in a browser and returns its ID.
    #[inline]
    pub async fn new_window(&self) -> Result<WebWindow> {
        self.0.lock().await.new_window().await
    }

    /// Switches to the provided [`WebWindow`] and executes the provided
    /// [`Statement`] in it.
    #[inline]
    pub async fn switch_to_window_and_execute(
        &self,
        window: WebWindow,
        exec: Statement,
    ) -> Result<Json> {
        self.0
            .lock()
            .await
            .switch_to_window_and_execute(window, exec)
            .await
    }

    /// Synchronously closes a [WebDriver] session.
    ///
    /// [WebDriver]: https://w3.org/TR/webdriver
    pub fn blocking_close(&self) {
        let (tx, rx) = mpsc::channel();
        let client = self.0.clone();
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
    pub fn blocking_window_close(&self, window: WebWindow) {
        let (tx, rx) = mpsc::channel();
        let client = self.0.clone();
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

/// Inner implementation of a [`WebDriverClient`].
struct Inner(Client);

impl Inner {
    /// Creates a new [WebDriver] session.
    ///
    /// [WebDriver]: https://w3.org/TR/webdriver
    pub async fn new() -> Result<Self> {
        Ok(Self(
            ClientBuilder::native()
                .capabilities(Self::get_webdriver_capabilities())
                .connect(&conf::WEBDRIVER_ADDR)
                .await?,
        ))
    }

    /// Executes the provided [`Statement`] in the current [`WebWindow`].
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
    pub async fn new_window(&mut self) -> Result<WebWindow> {
        let window = WebWindow(self.0.new_window(true).await?.handle);
        self.0.switch_to_window(window.clone()).await?;
        self.0
            .goto(&format!("http://{}/index.html", *conf::FILE_SERVER_HOST))
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
            let _ = self.0.close_window().await;
        }
    }

    /// Returns `moz:firefoxOptions` for a Firefox browser.
    fn get_firefox_caps() -> serde_json::Value {
        let mut args = FIREFOX_ARGS.to_vec();
        if *conf::HEADLESS {
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
    fn get_chrome_caps() -> serde_json::Value {
        let mut args = CHROME_ARGS.to_vec();
        if *conf::HEADLESS {
            args.push("--headless");
        }
        json!({ "args": args })
    }

    /// Returns [WebDriver capabilities][1] for Chrome and Firefox browsers.
    ///
    /// [1]: https:/mdn.io/Web/WebDriver/Capabilities
    fn get_webdriver_capabilities() -> Capabilities {
        let mut caps = Capabilities::new();
        caps.insert("moz:firefoxOptions".to_owned(), Self::get_firefox_caps());
        caps.insert("goog:chromeOptions".to_owned(), Self::get_chrome_caps());
        caps
    }
}
