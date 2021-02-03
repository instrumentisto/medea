//! Implementation of the object and methods for interracting with browser
//! through [WebDriver] protocol.
//!
//! [WebDriver]: https://www.w3.org/TR/webdriver/

use std::iter;

use derive_more::From;
use fantoccini::{Client, ClientBuilder, Locator};
use serde::Deserialize;
use serde_json::{json, Value as Json};
use webdriver::capabilities::Capabilities;

use crate::{conf, entity::Entity};

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
        c.goto(&format!("{}/index.html", *conf::FILE_SERVER_ADDR))
            .await?;
        c.wait_for_navigation(Some(
            format!("{}/index.html", *conf::FILE_SERVER_ADDR)
                .parse()
                .unwrap(),
        ))
        .await?;
        c.wait_for_find(Locator::Id("loaded")).await?;

        Ok(Self(c))
    }

    /// Returns `moz:firefoxOptions` for the Firefox browser based on
    /// [`TestRunner`] configuration.
    fn get_firefox_caps() -> serde_json::Value {
        json!({
            "prefs": {
                "media.navigator.streams.fake": true,
                "media.navigator.permission.disabled": true,
                "media.autoplay.enabled": true,
                "media.autoplay.enabled.user-gestures-needed ": false,
                "media.autoplay.ask-permission": false,
                "media.autoplay.default": 0,
            },
            "args": FIREFOX_ARGS,
        })
    }

    /// Returns `goog:chromeOptions` for the Chrome browser based on
    /// [`TestRunner`] configuration.
    fn get_chrome_caps() -> serde_json::Value {
        json!({ "args": CHROME_ARGS })
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
                            callback({{ err: e }});
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
}

pub struct JsExecutable {
    pub expression: String,
    pub args: Vec<Json>,
    pub objs: Vec<String>,
    pub and_then: Option<Box<JsExecutable>>,
    pub depth: u32,
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

    pub fn with_objs<T>(
        expression: &str,
        args: Vec<Json>,
        objs: Vec<&Entity<T>>,
    ) -> Self {
        Self {
            expression: expression.to_string(),
            args,
            objs: objs.into_iter().map(|o| o.id()).collect(),
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

    fn step_js(&self) -> String {
        format!(
            r#"
            args = arguments[{depth}];
            {objs_js}
            lastResult = await ({expr})(lastResult);
        "#,
            depth = self.depth,
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
        while let Some(mut e) = executable.take() {
            final_js.push_str(&e.step_js());
            args.push(std::mem::take(&mut e.args).into());
            executable = e.and_then;
        }

        (final_js, args)
    }
}
