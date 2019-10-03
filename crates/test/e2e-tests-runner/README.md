Medea's e2e tests runner
========================

This program is used for running e2e tests jobs in browser
with [WebDriver].

Runner load tests only from `e2e-tests` root project dir.

__All that is described below is needed for a better understanding of 
how everything works. If you just want to run the tests, 
then you just need to run `make test.e2e`.__

## Requirements

1) Running [WebDriver] (e.g. `chromedriver`, `geckodriver` etc)
2) Installed browsers (chrome for `chromedriver` or firefox for `geckodriver`)
3) Running [control-api-mock]
4) Running [medea]
5) Compiled [jason] with `--target web`

## Compile [jason] for tests
`$ wasm-pack build --target web --out-dir _dev/jason-pkg`

Out dir is very important for tests. Run this command in root of project
and don't forget run test runner also here.

## Flags
### `-f --files-host`
__Address where tests html files will be hosted.__

_If this flag is not specified then default value will be `localhost:9000`._

We need to start HTTP file server because browsers require valid MIME type for
loading some JS file with `type="module"`. Runner will serve all needed file
by itself but because you may be want to start parallel tests on different browser
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
__1. Run [coturn]__

`$ make up.coturn`

__2. Build [jason] with target web__

`$ cd jason && wasm-pack build --target web --out-dir .cache/jason-pkg && cd ../`

__3. Run [medea]__

`$ cargo run`

__4. Run [control-api-mock]__

`$ cargo run -p control-api-mock`

__5. Run some [WebDriver] ([chromedriver] in this example)__

`$ chromedriver -p 9515`

__6. Run all tests from `e2e-tests` root project dir__

`$ cargo run -p e2e-tests-runner -- -w http://localhost:9515 -f localhost:50000`

__7. Or you can run only one test__

`$ cargo run -p e2e-tests-runner -- -w http://localhost:9515 -f localhost:50000 pub_pub_video_call.spec.js`




[WebDriver]: https://developer.mozilla.org/en-US/docs/Web/WebDriver
[control-api-mock]: https://github.com/instrumentisto/medea/tree/master/control-api-mock
[medea]: https://github.com/instrumentisto/medea
[jason]: https://github.com/instrumentisto/medea/tree/master/jason
[chromedriver]: http://chromedriver.chromium.org/
[coturn]: https://github.com/coturn/coturn
