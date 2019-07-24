use fantoccini::{Client, Locator};
use futures::Future as _;
use serde_json::json;
use std::{
    fs::{canonicalize, File},
    io::prelude::*,
    path::PathBuf,
};
use webdriver::capabilities::Capabilities;

pub fn generate_html(test_js: &str) -> String {
    format!(include_str!("../test_template.html"), test_js)
}

pub fn generate_html_test(test_path: &PathBuf) {
    let mut file = File::open(test_path).unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content);
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
                .map(|_| {
                    // TODO: this is used for debug
                    println!("Tests passed!");
                    std::thread::sleep_ms(3000);
                })
                .map_err(|_| ()),
        );
    }
}
