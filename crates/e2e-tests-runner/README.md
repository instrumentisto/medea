Medea's E2E tests runner
========================

This application is used for running E2E tests jobs in a browser
with [WebDriver].

Runner loads tests only from `e2e-tests` root project dir.

__All that is described below is needed for a better understanding of 
how everything works. If you just want to run the tests, 
then you need to run `make test.e2e`.__




## Requirements

1) Running [WebDriver] (e.g. `chromedriver`, `geckodriver` etc)
2) Installed browsers (chrome for `chromedriver` or firefox for `geckodriver`)
3) Running [medea-control-api-mock]
4) Running [Medea]
5) Compiled [Jason] with `--target web`




## Compile [Jason] for tests

`$ wasm-pack build --target web --out-dir _dev/jason-pkg`

Out dir is very important for tests. Run this command in root of project
and don't forget to run test runner also here.




## Flags


### `-f --files-host`

__Address where tests HTML files will be hosted.__

_If this flag is not specified then default value will be `localhost:9000`._

We need to start HTTP file server because browsers requires valid MIME type for
loading some JS file with `type="module"`. Runner will serve all needed file
by itself, but because you may be want to start parallel tests on different browser
then you need specify different address for this HTTP file server.


### `-w --webdriver-addr` 

__Address to running [WebDriver]__

_If this flag is not specified then default value will be `localhost:4444`._

A runner does not matter for which browser the driver for this address is running.
The only thing that is important for the test runner to work is that the driver 
is compatible with the [WebDriver] protocol.


### `<test_file>`

__Run only specified test file from specs dir.__

_If nothing specified then runner will run all tests from specs dir_

If you wish run only one test file then you may specify this test name and
runner will run only this test.




## Example of usage

__1. Run [Coturn]__

`$ make up.coturn`

__2. Build [Jason] with target web__

`$ cd jason && wasm-pack build --target web --out-dir .cache/jason-pkg && cd ../`

__3. Run [Medea]__

`$ cargo run`

__4. Run [medea-control-api-mock]__

`$ cargo run -p medea-control-api-mock`

__5. Run some [WebDriver] ([chromedriver] in this example)__

`$ chromedriver -p 9515`

__6.1. Run all tests from `e2e-tests` root project dir__

`$ cargo run -p e2e-tests-runner -- -w http://localhost:9515 -f localhost:50000`

__6.2. Or you can run only one test__

`$ cargo run -p e2e-tests-runner -- -w http://localhost:9515 -f localhost:50000 e2e-tests/pub_pub_video_call.spec.js`

__6.3. Or you can run tests from a custom directory__

`$ cargo run -p e2e-tests-runner -- -w http://localhost:9515 -f localhost:50000 e2e-tests`




[WebDriver]: https://developer.mozilla.org/en-US/docs/Web/WebDriver
[medea-control-api-mock]: https://github.com/instrumentisto/medea/tree/master/mock/control-api
[Medea]: https://github.com/instrumentisto/medea
[Jason]: https://github.com/instrumentisto/medea/tree/master/jason
[chromedriver]: http://chromedriver.chromium.org/
[Coturn]: https://github.com/coturn/coturn
