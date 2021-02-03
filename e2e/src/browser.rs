use fantoccini::{Client, ClientBuilder, Locator};
use serde::Deserialize;
use serde_json::{json, Value as Json};
use webdriver::capabilities::Capabilities;

use crate::entity::Entity;

const CHROME_ARGS: &[&str] = &[
    "--use-fake-device-for-media-stream",
    "--use-fake-ui-for-media-stream",
    "--disable-web-security",
    "--disable-dev-shm-usage",
    "--no-sandbox",
];
const FIREFOX_ARGS: &[&str] = &[];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JsResult {
    Ok(Json),
    Err(Json),
}

impl From<JsResult> for Result<Json, Json> {
    fn from(from: JsResult) -> Self {
        match from {
            JsResult::Ok(ok) => Self::Ok(ok),
            JsResult::Err(err) => Self::Err(err),
        }
    }
}

#[derive(Clone, Debug)]
pub struct WebClient(Client);

impl WebClient {
    pub async fn new() -> Self {
        let mut c = ClientBuilder::native()
            .capabilities(Self::get_webdriver_capabilities())
            .connect("http://localhost:4444")
            .await
            .unwrap();
        c.goto("localhost:30000/index.html").await.unwrap();
        c.wait_for_navigation(Some(
            "localhost:30000/index.html".parse().unwrap(),
        ))
        .await
        .unwrap();
        c.wait_for_find(Locator::Id("loaded")).await.unwrap();

        Self(c)
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

    pub async fn execute(&mut self, executable: JsExecutable) -> Json {
        let (mut js, args) = executable.into_js();
        js.push_str("return lastResult;\n");

        self.0.execute(&js, args).await.unwrap()
    }

    pub async fn execute_async(
        &mut self,
        executable: JsExecutable,
    ) -> Result<Json, Json> {
        let mut js = "(async () => { try {".to_string();
        let (inner_js, args) = executable.into_js();
        js.push_str(&inner_js);
        js.push_str("arguments[arguments.length - 1]({ ok: lastResult });\n");
        js.push_str(
            r#"
        } catch (e) {
            let callback = arguments[arguments.length - 1];
            if (e.ptr != undefined) {
                callback({
                    err: {
                        name: e.name(),
                        message: e.message(),
                        trace: e.trace(),
                        source: e.source()
                    }
                });
            } else {
                callback({ err: e });
            }
        } } )();"#,
        );
        let res = self.0.execute_async(&js, args).await.unwrap();

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

    fn get_js_for_objs(&self) -> String {
        let mut objs = String::new();
        objs.push_str("objs = [];\n");
        for obj in &self.objs {
            objs.push_str(&format!(
                "objs.push(window.holders.get('{}'));\n",
                obj
            ));
        }

        objs
    }

    fn get_js(&self) -> String {
        let args = format!("args = arguments[{}];\n", self.depth);
        let objs = self.get_js_for_objs();
        let expr =
            format!("lastResult = await ({})(lastResult);\n", self.expression);

        let mut out = String::new();
        out.push_str(&args);
        out.push_str(&objs);
        out.push_str(&expr);

        out
    }

    fn into_js(self) -> (String, Vec<Json>) {
        let mut final_js = r#"
            let lastResult;
            let objs;
            let args;
        "#
        .to_string();
        let mut args = Vec::new();

        let mut executable = Some(Box::new(self));
        while let Some(mut e) = executable.take() {
            final_js.push_str(&e.get_js());
            args.push(std::mem::take(&mut e.args).into());
            executable = e.pop();
        }

        (final_js, args)
    }

    fn pop(self) -> Option<Box<Self>> {
        self.and_then
    }
}
