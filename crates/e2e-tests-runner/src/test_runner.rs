//! Implementation for run tests in a browser, check and print results.

// TODO: delete this when 'derive_builder' with #[automatically_derived] will be
//       released.
#![allow(clippy::default_trait_access)]

use std::{
    fs::{self, File},
    io::{prelude::*, Error as IoError},
    path::{Path, PathBuf},
};

use derive_builder::Builder;
use failure::Fail;
use fantoccini::{
    error::{CmdError, NewSessionError},
    Client, Locator,
};
use serde_json::json;
use webdriver::capabilities::Capabilities;

use crate::mocha_result::TestResults;

/// Errors which can occur in [`TestRunner`].
#[derive(Debug, Fail)]
pub enum Error {
    /// WebDriver command failed.
    #[fail(display = "WebDriver command failed: {:?}", _0)]
    Cmd(CmdError),

    /// WebDriver startup failed.
    #[fail(display = "WebDriver startup failed: {:?}", _0)]
    NewSession(NewSessionError),

    /// Test results not found in browser logs.
    #[fail(display = "Test results not found in browser logs. Probably \
                      something wrong with template. See printed browser \
                      logs for more info.")]
    TestResultsNotFoundInLogs,

    /// Some test failed.
    #[fail(display = "Some test failed.")]
    TestsFailed,
}

impl From<CmdError> for Error {
    fn from(err: CmdError) -> Self {
        Self::Cmd(err)
    }
}

impl From<NewSessionError> for Error {
    fn from(err: NewSessionError) -> Self {
        Self::NewSession(err)
    }
}

/// Medea's e2e tests runner.
///
/// Run e2e tests in browser, check results, print results.
#[derive(Builder)]
pub struct TestRunner<'a> {
    /// All paths to tests.
    #[builder(setter(skip))]
    tests: Vec<PathBuf>,

    /// Address where HTML test files will be hosted.
    test_files_host: &'a str,

    /// Don't close browser immediately on test fail. Browser will closed only
    /// on <Enter> press.
    is_wait_on_fail_mode: bool,

    /// URL to a WebDriver with which tests will be run.
    webdriver_addr: &'a str,

    /// If `true` then tests will be run in a headless browser.
    is_headless: bool,
}

impl<'a> TestRunner<'a> {
    /// Starts running E2E tests from the provided [`PathBuf`].
    pub async fn run(mut self, path_to_tests: PathBuf) -> Result<(), Error> {
        let (tests, test_dir) = if path_to_tests.is_dir() {
            (
                get_paths_to_tests_from_dir(&path_to_tests).unwrap(),
                path_to_tests.as_path(),
            )
        } else {
            (vec![path_to_tests.clone()], path_to_tests.parent().unwrap())
        };
        self.tests = tests;

        let result = self.run_tests().await;
        delete_all_tests_htmls(test_dir).unwrap();

        result
    }

    /// Returns new [`Client`] with which E2E tests will be run by
    /// [`TestRunner`].
    async fn get_client(&self) -> Client {
        Client::with_capabilities(
            self.webdriver_addr,
            self.get_webdriver_capabilities(),
        )
        .await
        .unwrap()
    }

    /// Creates WebDriver client, starts E2E tests loop.
    ///
    /// When all tests finished then created WebDriver [`Client`] will be
    /// closed.
    async fn run_tests(&mut self) -> Result<(), Error> {
        let mut client = self.get_client().await;

        let result = self.run_tests_loop(&mut client).await;
        if result.is_err() {
            if self.is_wait_on_fail_mode {
                wait_for_enter();
            }
            client.close().await?;
        }

        result
    }

    /// Runs single test from provided [`PathBuf`].
    ///
    /// Provided [`PathBuf`] must point to a __file__.
    async fn run_test(
        &mut self,
        client: &mut Client,
        test: &PathBuf,
    ) -> Result<(), Error> {
        let test_path = generate_and_save_test_html(test);
        let test_url = self.get_url_to_test(&test_path);
        println!(
            "\nRunning {} test...",
            test.file_name().unwrap().to_str().unwrap()
        );

        client.goto(&test_url).await?;
        wait_for_test_end(client).await?;
        self.check_test_results(client).await?;

        Ok(())
    }

    /// Tests loop which alternately launches tests in a browser.
    ///
    /// This [`Future`] will be resolved when all tests completed or when some
    /// test failed.
    ///
    /// Returns [`Error::TestsFailed`] if some test failed.
    async fn run_tests_loop(
        &mut self,
        client: &mut Client,
    ) -> Result<(), Error> {
        while let Some(test) = self.tests.pop() {
            self.run_test(client, &test).await?;
        }

        Ok(())
    }

    /// Checks result of tests.
    ///
    /// This function will close WebDriver's session if some error happen.
    ///
    /// Returns [`Error::TestsFailed`] if some test failed.
    ///
    /// Returns [`Error::TestResultsNotFoundInLogs`] if [Mocha] results not
    /// found in JS side console logs.
    ///
    /// [Mocha]: https://mochajs.org/
    async fn check_test_results(
        &mut self,
        client: &mut Client,
    ) -> Result<(), Error> {
        let errors = client
            .execute("return console.logs", Vec::new())
            .await
            .unwrap();
        let logs = errors.as_array().unwrap();

        for message in logs {
            let message = message.as_array().unwrap()[0].as_str().unwrap();
            if let Ok(test_results) =
                serde_json::from_str::<TestResults>(message)
            {
                println!("{}", test_results);

                return if test_results.is_has_error() {
                    println!("Console log: ");
                    logs.iter()
                        .flat_map(|msg| msg.as_array().unwrap().iter())
                        .for_each(|msg| println!("{}", msg.as_str().unwrap()));

                    Err(Error::TestsFailed)
                } else {
                    Ok(())
                };
            }
        }

        Err(Error::TestResultsNotFoundInLogs)
    }

    /// Returns URL to a test based on provided [`PathBuf`] to this test.
    fn get_url_to_test(&self, test_path: &PathBuf) -> String {
        let filename = test_path.file_name().unwrap().to_str().unwrap();

        format!("http://{}/e2e-tests/{}", self.test_files_host, filename)
    }

    /// Returns [Chrome] arguments based on [`TestRunner`] configuration.
    ///
    /// [Chrome]: https://www.google.com/chrome/
    fn get_chrome_args(&self) -> Vec<&str> {
        let mut default_args = vec![
            "--use-fake-device-for-media-stream",
            "--use-fake-ui-for-media-stream",
            "--disable-web-security",
            "--disable-dev-shm-usage",
            "--no-sandbox",
        ];
        if self.is_headless {
            default_args.push("--headless");
        }

        default_args
    }

    /// Returns [Firefox] arguments based on [`TestRunner`] configuration.
    ///
    /// [Firefox]: https://www.mozilla.org/en-US/firefox/
    fn get_firefox_args(&self) -> Vec<&str> {
        let mut default_args = vec![];
        if self.is_headless {
            default_args.push("--headless");
        }

        default_args
    }

    /// Returns `moz:firefoxOptions` for the Firefox browser based on
    /// [`TestRunner`] configuration.
    fn get_firefox_caps(&self) -> serde_json::Value {
        json!({
            "prefs": {
                "media.navigator.streams.fake": true,
                "media.navigator.permission.disabled": true,
                "media.autoplay.enabled": true,
                "media.autoplay.enabled.user-gestures-needed ": false,
                "media.autoplay.ask-permission": false,
                "media.autoplay.default": 0,
            },
            "args": self.get_firefox_args()
        })
    }

    /// Returns `goog:chromeOptions` for the Chrome browser based on
    /// [`TestRunner`] configuration.
    fn get_chrome_caps(&self) -> serde_json::Value {
        json!({ "args": self.get_chrome_args() })
    }

    /// Returns [WebDriver capabilities] based on [`TestRunner`] configuration.
    ///
    /// [WebDriver capabilities]:
    /// https://developer.mozilla.org/en-US/docs/Web/WebDriver/Capabilities
    fn get_webdriver_capabilities(&self) -> Capabilities {
        let mut capabilities = Capabilities::new();
        capabilities
            .insert("moz:firefoxOptions".to_string(), self.get_firefox_caps());
        capabilities
            .insert("goog:chromeOptions".to_string(), self.get_chrome_caps());

        capabilities
    }
}

// TODO: change to anykey
/// Locks thread until used presses `Enter` in command line.
fn wait_for_enter() {
    let mut s = String::new();
    println!("Press <Enter> for close...");
    std::io::stdin().read_line(&mut s).unwrap();
}

/// Returns relative URL to a provided helper [`PathBuf`].
fn helper_url(path: &PathBuf) -> String {
    let filename = path.file_name().unwrap().to_str().unwrap();
    format!("/e2e-tests/helper/{}", filename)
}

/// Returns relative URLs to all tests helpers from `e2e-tests/helper`.
fn helpers_urls() -> Result<Vec<String>, IoError> {
    let mut test_path = crate::get_default_path_to_tests();
    test_path.push("helper");

    Ok(fs::read_dir(test_path)?
        .filter_map(|e| e.ok().map(|e| helper_url(&e.path())))
        .collect())
}

/// Generates HTML for spec with `test_template.html` template.
///
/// Note that template will be included into binary at compile time.
/// When you change `test_template.html`, you must recompile project for the
/// changes to take effect.
fn generate_test_html(test_name: &str) -> String {
    const DONT_EDIT_WARNING: &str =
        "<!--DON'T EDIT THIS FILE. THIS IS AUTOGENERATED FILE FOR TESTS-->";
    let html_body =
        include_str!("../test_template.html").replace("{{{test}}}", test_name);

    let helpers_include: String = helpers_urls()
        .unwrap()
        .into_iter()
        .map(|helper_url| format!(r#"<script src="{}"></script>"#, helper_url))
        .collect();
    let html_body = html_body.replace("<helpers/>", &helpers_include);

    format!("{}\n{}", DONT_EDIT_WARNING, html_body)
}

/// Generates HTML and save it with same path as a spec but with
/// `.html` extension.
fn generate_and_save_test_html(test_path: &PathBuf) -> PathBuf {
    let test_html =
        generate_test_html(test_path.file_name().unwrap().to_str().unwrap());

    let html_test_file_path = test_path.with_extension("html");
    let mut file = File::create(&html_test_file_path).unwrap();
    file.write_all(test_html.as_bytes()).unwrap();

    html_test_file_path
}

/// This [`Future`] will be resolved when element with `test-end` ID appears on
/// page.
async fn wait_for_test_end(client: &mut Client) -> Result<(), CmdError> {
    client.wait_for_find(Locator::Id("test-end")).await?;
    Ok(())
}

/// Returns `true` if provided [`PathBuf`] is pointing to a JS file.
fn is_js_file(path: &PathBuf) -> bool {
    let is_js_ext = path.extension().filter(|ext| *ext == "js").is_some();
    path.is_file() && is_js_ext
}

/// Returns all [`PathBuf`]s to tests files from provided directory [`PathBuf`].
///
/// [`PathBuf`] provided into this function must be path to a directory.
fn get_paths_to_tests_from_dir(
    path_to_test_dir: &PathBuf,
) -> Result<Vec<PathBuf>, IoError> {
    Ok(fs::read_dir(path_to_test_dir)?
        .filter_map(|r| r.ok().map(|e| e.path()))
        .filter(|path| is_js_file(path))
        .collect())
}

/// Delete all generated tests HTML from test dir.
#[allow(clippy::filter_map)]
fn delete_all_tests_htmls(path_test_dir: &Path) -> Result<(), IoError> {
    fs::read_dir(path_test_dir)?
        .filter_map(|r| r.ok().map(|p| p.path()))
        .filter(|path| path.extension().map_or(false, |ext| ext == "html"))
        .map(fs::remove_file)
        .collect()
}
