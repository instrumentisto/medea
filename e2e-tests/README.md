E2E testing of medea
====================

__Run E2E tests:__ `$ make test.e2e`

## How to write e2e tests for medea
In this directory are located all medea's E2E tests (except signalling tests).
All tests from this directory will be running by [e2e-tests-runner]. 

On JS side for testing we use [mocha] with [chai].
Therefore, for a start it is worth reading the basic guide on them.

This tests will be running in real browser (`chromium`, `firefox`) by
[e2e-tests-runner] and all results will be printed in terminal.

Also in every test you can use already writen helpers from helpers path.
All files from `e2e-tests/helpers` will be included into test html as `<script>`.
In [test_template.html] file you may add whatever you want (script, css, text). 
This is a file in the context of which all tests will run.

For interacting with [jason] you may call async function `window.getJason()`.
This function will return new `Jason` object with whom you can interact as you wish.

All tests running exactly on browser side. Because of that you may interact directly
with all you need.

Also all supported by [e2e-tests-runner] browsers will start with mocked `userMedia`.
If you wish add some flag/pref to some browser than you need edit 
`get_webdriver_capabilities` in [e2e-tests-runner]'s `test_runner` module.

__Also recommended read README for [e2e-tests-runner] for better understanding how
it works.__




[e2e-tests-runner]: https://github.com/instrumentisto/medea/tree/master/crates/test/e2e-tests-runner
[jason]: https://github.com/instrumentisto/medea/tree/master/jason
[mocha]: https://mochajs.org/
[chai]: https://www.chaijs.com/
[test_template.html]: https://github.com/instrumentisto/medea/blob/master/crates/test/e2e-tests-runner/test_template.html
