E2E testing of Medea
====================

__Run E2E tests:__ `$ make test.e2e`




## How to write E2E tests for Medea

In this directory are located all medea's E2E tests (except signalling tests).
All tests from this directory will be ran by [e2e-tests-runner]. 

On JS side for testing we use [Mocha] and [Chai].
Therefore, for a start it is worth reading the basic guide on them.

This tests will be running in a real browser (`chromium`, `firefox`) by
[e2e-tests-runner] and all results will be printed in terminal.

Also in every test you can use already written helpers from helpers path.
All files from `e2e-tests/helpers` will be included into test html as `<script>`.
In [test_template.html] file you may add whatever you want (script, css, text). 
This is a file in the context of which all tests will be ran.

For interacting with [Jason] you may call async function `window.getJason()`.
This function will return new `Jason` object with whom you can interact as you wish.

All tests will be ran exactly on browser side. Because of that you may interact directly
with all you need.

All supported by [e2e-tests-runner] browsers will be started with mocked `userMedia`.
If you wish to add some flags/prefs to some browser than you need edit 
`get_webdriver_capabilities` in a [e2e-tests-runner]'s `test_runner` module.

__Also recommended read README for [e2e-tests-runner] for better understanding how
it works.__




[e2e-tests-runner]: https://github.com/instrumentisto/medea/tree/master/crates/e2e-tests-runner
[Jason]: https://github.com/instrumentisto/medea/tree/master/jason
[Mocha]: https://mochajs.org/
[Chai]: https://www.chaijs.com/
[test_template.html]: https://github.com/instrumentisto/medea/blob/master/crates/e2e-tests-runner/test_template.html
