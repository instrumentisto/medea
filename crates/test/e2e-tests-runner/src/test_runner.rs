//! Implementation for run tests in browser, check and print results.

use std::{
    fs::File,
    io::{prelude::*, Error as IoError},
    path::{Path, PathBuf},
};

use clap::ArgMatches;
use derive_builder::Builder;
use failure::Fail;
use fantoccini::{
    error::{CmdError, NewSessionError},
    Client, Locator,
};
use futures::Future;
use serde_json::json;
use webdriver::capabilities::Capabilities;

use crate::mocha_result::TestResults;

/// Errors which can occur in [`TestRunner`].
#[allow(clippy::pub_enum_variant_names)]
#[derive(Debug, Fail)]
pub enum Error {
    /// WebDriver command failed.
    #[fail(display = "WebDriver command failed: {:?}", _0)]
    CmdErr(CmdError),

    /// WebDriver startup failed.
    #[fail(display = "WebDriver startup failed: {:?}", _0)]
    NewSessionError(NewSessionError),

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
        Self::CmdErr(err)
    }
}

impl From<NewSessionError> for Error {
    fn from(err: NewSessionError) -> Self {
        Self::NewSessionError(err)
    }
}

/// Delete all generated tests html from test dir.
fn delete_all_tests_htmls(path_test_dir: &Path) -> Result<(), IoError> {
    for entry in std::fs::read_dir(path_test_dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "html" {
                std::fs::remove_file(path)?;
            }
        }
    }
    Ok(())
}

/// Medea's e2e tests runner.
///
/// Run e2e tests in browser, check results, print results.
#[derive(Builder)]
pub struct TestRunner<'a> {
    #[builder(setter(skip))]
    /// All paths to tests.
    tests: Vec<PathBuf>,

    /// Address where html test files will be hosted.
    test_addr: &'a str,

    /// Don't close browser immediately on test fail. Browser will closed only
    /// on <Enter> press.
    is_wait_on_fail_mode: bool,

    webdriver_addr: &'a str,

    is_headless: bool,
}

impl<'a> TestRunner<'a> {
    /// Run e2e tests.
    pub async fn run(mut self, path_to_tests: PathBuf) -> Result<(), Error> {
        let (tests, test_dir) = if path_to_tests.is_dir() {
            (get_all_tests_paths(&path_to_tests), path_to_tests.as_path())
        } else {
            (vec![path_to_tests.clone()], path_to_tests.parent().unwrap())
        };
        self.tests = tests;
        let result = self.run_tests().await;
        delete_all_tests_htmls(test_dir).unwrap();
        result
    }

    async fn get_client(&self) -> Client {
        let caps = self.get_webdriver_capabilities();
        Client::with_capabilities(self.webdriver_addr, caps)
            .await
            .unwrap()
    }

    /// Create WebDriver client, start e2e tests loop.
    async fn run_tests(&mut self) -> Result<(), Error> {
        let mut client = self.get_client().await;
        let result = self.tests_loop(&mut client).await;
        if result.is_err() {
            if self.is_wait_on_fail_mode {
                let mut s = String::new();
                println!("Press <Enter> for close...");
                std::io::stdin().read_line(&mut s).unwrap();
            }
            client.close().await?;
        }
        result
    }

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

    /// Tests loop which alternately launches tests in browser.
    ///
    /// This future resolve when all tests completed or when test failed.
    ///
    /// Returns [`Error::TestsFailed`] if some test failed.
    async fn tests_loop(&mut self, client: &mut Client) -> Result<(), Error> {
        loop {
            if let Some(test) = self.tests.pop() {
                self.run_test(client, &test).await?;
            } else {
                break;
            }
        }
        Ok(())
    }

    /// Check results of tests.
    ///
    /// This function will close WebDriver's session if some error happen.
    ///
    /// Returns [`Error::TestsFailed`] if some test failed.
    ///
    /// Returns [`Error::TestResultsNotFoundInLogs`] if mocha results not found
    /// in JS side console logs.
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
                    for messages in logs {
                        let messages = messages.as_array().unwrap();
                        for message in messages {
                            let message = message.as_str().unwrap();
                            println!("{}", message);
                        }
                    }
                    Err(Error::TestsFailed)
                } else {
                    Ok(())
                };
            }
        }

        Err(Error::TestResultsNotFoundInLogs)
    }

    /// Returns url which runner will open.
    fn get_url_to_test(&self, test_path: &PathBuf) -> String {
        let filename = test_path.file_name().unwrap().to_str().unwrap();
        format!("http://{}/e2e-tests/{}", self.test_addr, filename)
    }

    /// Returns browser capabilities based on arguments.
    ///
    /// Currently check `--headless` flag and based on this run headed or
    /// headless browser.
    fn get_webdriver_capabilities(&self) -> Capabilities {
        let mut capabilities = Capabilities::new();

        let mut firefox_args = Vec::new();
        let mut chrome_args = vec![
            "--use-fake-device-for-media-stream",
            "--use-fake-ui-for-media-stream",
            "--disable-web-security",
            "--disable-dev-shm-usage",
            "--no-sandbox",
        ];
        if self.is_headless {
            firefox_args.push("--headless");
            chrome_args.push("--headless");
        }

        let firefox_settings = json!({
            "prefs": {
                "media.navigator.streams.fake": true,
                "media.navigator.permission.disabled": true,
                "media.autoplay.enabled": true,
                "media.autoplay.enabled.user-gestures-needed ": false,
                "media.autoplay.ask-permission": false,
                "media.autoplay.default": 0,
            },
            "args": firefox_args
        });
        capabilities.insert("moz:firefoxOptions".to_string(), firefox_settings);

        let chrome_settings = json!({ "args": chrome_args });
        capabilities.insert("goog:chromeOptions".to_string(), chrome_settings);

        capabilities
    }
}

/// Returns urls to all helpers JS from `e2e-tests/helper`.
fn get_all_helpers_urls() -> Result<Vec<String>, IoError> {
    let mut test_path = crate::get_path_to_tests();
    let mut helpers = Vec::new();
    test_path.push("helper");
    for entry in std::fs::read_dir(test_path)? {
        let entry = entry?;
        let path = entry.path();
        helpers.push(path);
    }

    Ok(helpers
        .into_iter()
        .map(|f| {
            format!(
                "/e2e-tests/helper/{}",
                f.file_name().unwrap().to_str().unwrap()
            )
        })
        .collect())
}

/// Generate html for spec by `test_template.html` from root.
fn generate_test_html(test_name: &str) -> String {
    let dont_edit_warning = "<!--DON'T EDIT THIS FILE. THIS IS AUTOGENERATED \
                             FILE FOR TESTS-->"
        .to_string();
    let html_body =
        include_str!("../test_template.html").replace("{{{test}}}", test_name);

    let mut helpers_include = String::new();
    for helper_url in get_all_helpers_urls().unwrap() {
        helpers_include
            .push_str(&format!(r#"<script src="{}"></script>"#, helper_url));
    }
    let html_body = html_body.replace("<helpers/>", &helpers_include);

    format!("{}\n{}", dont_edit_warning, html_body)
}

/// Generate html and save it with same path as a spec but with extension
/// `.html`.
fn generate_and_save_test_html(test_path: &PathBuf) -> PathBuf {
    let test_html =
        generate_test_html(test_path.file_name().unwrap().to_str().unwrap());

    let html_test_file_path = test_path.with_extension("html");
    let mut file = File::create(&html_test_file_path).unwrap();
    file.write_all(test_html.as_bytes()).unwrap();

    html_test_file_path
}

/// This future resolve when div with ID `test-end` appear on page.
async fn wait_for_test_end(client: &mut Client) -> Result<(), CmdError> {
    client.wait_for_find(Locator::Id("test-end")).await?;
    Ok(())
}

/// Get all paths to spec files from provided dir.
fn get_all_tests_paths(path_to_test_dir: &PathBuf) -> Vec<PathBuf> {
    let mut tests_paths = Vec::new();
    for entry in std::fs::read_dir(path_to_test_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "js" {
                    tests_paths.push(path);
                }
            }
        }
    }
    tests_paths
}
