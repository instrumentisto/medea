//! Implementation of client for the [WebDriver].
//!
//! [WebDriver]: https://www.w3.org/TR/webdriver/

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

/// Result which will be returned from the all JS code executed in browser.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum JsResult {
    /// Contains the success [`Json`] value.
    Ok(Json),

    /// Contains the error [`Json`] value.
    Err(Json),
}

impl From<JsResult> for Result<Json> {
    fn from(from: JsResult) -> Self {
        match from {
            JsResult::Ok(ok) => Self::Ok(ok),
            JsResult::Err(err) => Self::Err(Error::Js(err)),
        }
    }
}

/// Client for interacting with browser through [WebDriver].
///
/// [WebDriver]: https://www.w3.org/TR/webdriver/
#[derive(Clone, Debug)]
pub struct WebDriverClient(Arc<Mutex<Inner>>);

impl WebDriverClient {
    /// Returns new [`WebClient`] connected to the [WebDriver].
    ///
    /// [WebDriver]: https://www.w3.org/TR/webdriver/
    pub async fn new() -> Result<Self> {
        Ok(Self(Arc::new(Mutex::new(Inner::new().await?))))
    }

    /// Creates new window and returns it's ID.
    pub async fn new_window(&self) -> Result<WebWindow> {
        self.0.lock().await.new_window().await
    }

    /// Switches to the provided [`WebWindow`] and executes provided
    /// [`Statement`] in it.
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

    /// Synchronously closes [WebDriver] session.
    ///
    /// [WebDriver]: https://www.w3.org/TR/webdriver/
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

    /// Synchronously closes provided [`WebWindow`].
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

/// Inner for the [`WebClient`].
struct Inner(Client);

impl Inner {
    /// Creates new [WebDriver] session.
    ///
    /// [WebDriver]: https://www.w3.org/TR/webdriver/
    pub async fn new() -> Result<Self> {
        let c = ClientBuilder::native()
            .capabilities(Self::get_webdriver_capabilities())
            .connect(&conf::WEBDRIVER_ADDR)
            .await?;

        Ok(Self(c))
    }

    /// Executes provided [`Statement`] in the current [`WebWindow`].
    pub async fn execute(&mut self, statement: Statement) -> Result<Json> {
        let (inner_js, args) = statement.prepare();

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
            executable_js = inner_js
        );
        let res = self.0.execute_async(&js, args).await?;

        serde_json::from_value::<JsResult>(res)?.into()
    }

    /// Creates new [`WebWindow`] and returns it's ID.
    ///
    /// Creates `registry` in the created [`WebWindow`].
    pub async fn new_window(&mut self) -> Result<WebWindow> {
        let window = WebWindow(self.0.new_window(true).await?.handle);
        self.0.switch_to_window(window.clone()).await?;
        self.0
            .goto(&format!("http://{}/index.html", *conf::FILE_SERVER_ADDR))
            .await?;
        self.0.wait_for_find(Locator::Id("loaded")).await?;

        self.execute(Statement::new(
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

    /// Switches to the provided [`WebWindow`] and executes provided
    /// [`Statement`].
    pub async fn switch_to_window_and_execute(
        &mut self,
        window: WebWindow,
        exec: Statement,
    ) -> Result<Json> {
        self.0.switch_to_window(window).await?;

        Ok(self.execute(exec).await?)
    }

    /// Closes provided [`WebWindow`].
    pub async fn close_window(&mut self, window: WebWindow) {
        if self.0.switch_to_window(window).await.is_ok() {
            let _ = self.0.close_window().await;
        }
    }

    /// Returns `moz:firefoxOptions` for the Firefox browser.
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

    /// Returns `goog:chromeOptions` for the Chrome browser.
    fn get_chrome_caps() -> serde_json::Value {
        let mut args = CHROME_ARGS.to_vec();
        if *conf::HEADLESS {
            args.push("--headless");
        }
        json!({ "args": args })
    }

    /// Returns [WebDriver capabilities] for Chrome and Firefox browsers.
    ///
    /// [WebDriver capabilities]:
    /// https://developer.mozilla.org/en-US/docs/Web/WebDriver/Capabilities
    fn get_webdriver_capabilities() -> Capabilities {
        let mut capabilities = Capabilities::new();
        capabilities
            .insert("moz:firefoxOptions".to_string(), Self::get_firefox_caps());
        capabilities
            .insert("goog:chromeOptions".to_string(), Self::get_chrome_caps());

        capabilities
    }
}
