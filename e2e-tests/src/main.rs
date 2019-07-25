use std::{
    fmt,
    fs::{canonicalize, File},
    io::prelude::*,
    path::PathBuf,
};

use fantoccini::{Client, Locator};
use futures::Future as _;
use serde::Deserialize;
use serde_json::json;
use webdriver::capabilities::Capabilities;
use yansi::Paint;

pub fn generate_html(test_js: &str) -> String {
    include_str!("../test_template.html").replace("{{{}}}", test_js)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestStats {
    suites: i32,
    tests: i32,
    passes: i32,
    pending: i32,
    failures: i32,
    start: String,
    end: String,
    duration: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestError {
    message: Option<String>,
    show_diff: Option<bool>,
    actual: Option<String>,
    expected: Option<String>,
    stack: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestResult {
    title: String,
    full_title: String,
    duration: u32,
    err: TestError,
}

impl fmt::Display for TestResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.err.message.is_some() {
            write!(
                f,
                "   {}\n\n",
                Paint::red(format!(
                    "test {} ... failed ({} ms)",
                    self.full_title, self.duration
                ))
            )?;
            write!(f, "   Message: {}", self.err.message.as_ref().unwrap())?;
            if let Some(stack) = &self.err.stack {
                write!(f, "\n   Stacktrace:\n\n   {}\n\n", stack)?;
            }
        } else {
            write!(
                f,
                "   {}\n",
                Paint::green(format!(
                    "test {} ... ok ({} ms)",
                    self.full_title, self.duration
                ))
            )?;
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestResults {
    stats: TestStats,
    tests: Vec<TestResult>,
    failures: Vec<TestResult>,
    passes: Vec<TestResult>,
}

impl TestResults {
    pub fn is_has_error(&self) -> bool {
        !self.failures.is_empty()
    }
}

impl fmt::Display for TestResults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\nPassed tests ({}):\n\n", self.stats.passes)?;
        for passed in &self.passes {
            write!(f, "{}\n", passed)?;
        }

        if !self.failures.is_empty() {
            write!(f, "\nFailed tests ({}):\n\n", self.stats.failures)?;
            for failure in &self.failures {
                write!(f, "{}", failure)?;
            }
        }

        write!(f, "{}", Paint::yellow("Summary: "))?;
        write!(f, "suites: {}; ", self.stats.suites)?;
        write!(f, "tests: {}; ", self.stats.tests)?;
        write!(f, "passes: {}; ", self.stats.passes)?;
        write!(f, "failures: {}.\n", self.stats.failures)?;

        Ok(())
    }
}

pub fn generate_html_test(test_path: &PathBuf) {
    let mut file = File::open(test_path).unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    let mut file = File::create("test.html").unwrap();
    let test_html = generate_html(&content);
    file.write_all(test_html.as_bytes()).unwrap();
}

fn main() {
    let path_to_tests = std::env::args().skip(1).next().unwrap();
    let path_to_tests = PathBuf::from(path_to_tests);
    let path_to_tests = canonicalize(path_to_tests).unwrap();

    generate_html_test(&path_to_tests);

    let mut capabilities = Capabilities::new();
    let firefox_settings = json!({
        "prefs": {
            "media.navigator.streams.fake": true
        }
    });
    capabilities.insert("moz:firefoxOptions".to_string(), firefox_settings);

    // TODO: chrome
    {
        let chrome_settings = json!({
            "args": [
                "--use-fake-device-for-media-stream",
                "--use-fake-ui-for-media-stream"
            ]
        });
        capabilities.insert("goog:chromeOptions".to_string(), chrome_settings);
    }

    if path_to_tests.is_dir() {
        unimplemented!("dir")
    } else {
        let client =
            Client::with_capabilities("http://localhost:9515", capabilities);
        //        let test_url = format!("file://{}", path_to_tests.display());
        let test_url =
            "file:///home/relmay/Projects/work/medea/e2e-tests/test.html";

        tokio::run(
            client
                .map_err(|e| panic!("{:?}", e))
                .and_then(move |client| client.goto(&test_url))
                .and_then(|client| {
                    client.wait_for_find(Locator::Id("test-end"))
                })
                .map(|e| e.client())
                .and_then(|mut client| {
                    client.execute("return console.logs[0][0]", Vec::new())
                })
                .map(|result| {
                    let result = result.as_str().unwrap();
                    let result: TestResults =
                        serde_json::from_str(&result).unwrap();
                    result
                })
                .map(|result| {
                    println!("{}", result);
                    if result.is_has_error() {
                        std::process::exit(1);
                    }
                })
                .map_err(|_| ()),
        );
    }
}
