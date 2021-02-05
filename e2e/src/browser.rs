//! Implementation of the object and methods for interracting with browser
//! through [WebDriver] protocol.
//!
//! [WebDriver]: https://www.w3.org/TR/webdriver/

use std::{
    iter,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use derive_more::From;
use fantoccini::{Client, ClientBuilder, Locator};
use serde::Deserialize;
use serde_json::{json, Value as Json};
use webdriver::{capabilities::Capabilities, common::WebWindow};

use crate::{conf, entity::EntityPtr};

const CHROME_ARGS: &[&str] = &[
    "--use-fake-device-for-media-stream",
    "--use-fake-ui-for-media-stream",
    "--disable-web-security",
    "--disable-dev-shm-usage",
    "--no-sandbox",
];
const FIREFOX_ARGS: &[&str] = &[];

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

    async fn switch_window(&mut self) {
        self.client.switch_to_window(self.window.clone()).await;
    }

    pub async fn execute(&mut self, exec: JsExecutable) -> Result<Json> {
        self.switch_window().await;

        self.client.execute(exec).await
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

/// Client for interacting with browser through WebDriver.
#[derive(Clone, Debug)]
pub struct WebClient(Client);

impl WebClient {
    /// Returns new [`WebClient`] connected to the WebDriver
    pub async fn new() -> Result<Self> {
        let mut c = ClientBuilder::native()
            .capabilities(Self::get_webdriver_capabilities())
            .connect(&conf::WEBDRIVER_ADDR)
            .await?;

        Ok(Self(c))
    }

    /// Returns `moz:firefoxOptions` for the Firefox browser based on
    /// [`TestRunner`] configuration.
    fn get_firefox_caps() -> serde_json::Value {
        let mut args = CHROME_ARGS.to_vec();
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

    /// Returns `goog:chromeOptions` for the Chrome browser based on
    /// [`TestRunner`] configuration.
    fn get_chrome_caps() -> serde_json::Value {
        let mut args = CHROME_ARGS.to_vec();
        if *conf::HEADLESS {
            args.push("--headless");
        }
        json!({ "args": args })
    }

    /// Returns [WebDriver capabilities] based on [`TestRunner`] configuration.
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

    pub async fn execute(&mut self, executable: JsExecutable) -> Result<Json> {
        let (inner_js, args) = executable.finalize();

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

        serde_json::from_value::<JsResult>(res).unwrap().into()
    }

    pub async fn new_window(&mut self) -> WebWindow {
        let window = WebWindow(self.0.new_window(true).await.unwrap().handle);
        self.switch_to_window(window.clone()).await;
        self.0
            .goto(&format!("http://{}/index.html", *conf::FILE_SERVER_ADDR))
            .await
            .unwrap();
        self.0.wait_for_find(Locator::Id("loaded")).await.unwrap();
        self.execute(JsExecutable::new(
            r#"
                async () => {
                    window.holders = new Map();
                }
            "#,
            vec![],
        ))
        .await
        .unwrap();

        window
    }

    pub async fn switch_to_window(&mut self, window: WebWindow) {
        self.0.switch_to_window(window).await.unwrap();
    }

    pub fn blocking_close(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut client = self.0.clone();
        tokio::spawn(async move {
            let _ = client.close().await;
            tx.send(()).unwrap();
        });
        rx.recv().unwrap();
    }

    pub fn blocking_window_close(&mut self, window: WebWindow) {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut client = self.0.clone();
        tokio::spawn(async move {
            client.switch_to_window(window).await.unwrap();
            let _ = client.close_window().await;
            tx.send(()).unwrap();
        });
        rx.recv().unwrap();
    }
}

pub struct JsExecutable {
    expression: String,
    args: Vec<Json>,
    objs: Vec<EntityPtr>,
    and_then: Option<Box<JsExecutable>>,
    depth: u32,
}

impl JsExecutable {
    pub fn new(expression: &str, args: Vec<Json>) -> Self {
        Self {
            expression: expression.to_string(),
            args,
            objs: Vec::new(),
            and_then: None,
            depth: 0,
        }
    }

    pub fn with_objs(
        expression: &str,
        args: Vec<Json>,
        objs: Vec<EntityPtr>,
    ) -> Self {
        Self {
            expression: expression.to_string(),
            args,
            objs,
            and_then: None,
            depth: 0,
        }
    }

    pub fn and_then(mut self, mut another: Self) -> Self {
        if let Some(e) = self.and_then {
            self.and_then = Some(Box::new(e.and_then(another)));
            self
        } else {
            another.depth = self.depth + 1;
            self.and_then = Some(Box::new(another));
            self
        }
    }

    fn objects_injection_js(&self) -> String {
        iter::once("objs = [];\n".to_string())
            .chain(self.objs.iter().map(|id| {
                format!("objs.push(window.holders.get('{}'));\n", id)
            }))
            .collect()
    }

    fn step_js(&self, i: usize) -> String {
        format!(
            r#"
            args = arguments[{depth}];
            {objs_js}
            lastResult = await ({expr})(lastResult);
        "#,
            depth = i,
            objs_js = self.objects_injection_js(),
            expr = self.expression
        )
    }

    fn finalize(self) -> (String, Vec<Json>) {
        let mut final_js = r#"
            let lastResult;
            let objs;
            let args;
        "#
        .to_string();
        let mut args = Vec::new();

        let mut executable = Some(Box::new(self));
        let mut i = 0;
        while let Some(mut e) = executable.take() {
            final_js.push_str(&e.step_js(i));
            i += 1;
            args.push(std::mem::take(&mut e.args).into());
            executable = e.and_then;
        }

        (final_js, args)
    }
}
