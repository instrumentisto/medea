//! Objects which will be returned from [Mocha] tests.
//!
//! [Mocha]: https://mochajs.org/

use std::fmt;

use serde::Deserialize;
use yansi::Paint;

/// Results of [Mocha] tests.
///
/// This struct will be parsed from [Mocha]'s JSON reporters string.
///
/// [Mocha]: https://mochajs.org/
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResults {
    /// Summary stats of all tests.
    pub stats: TestStats,

    /// All failed tests.
    pub failures: Vec<FailureTestResult>,

    /// All success tests.
    pub passes: Vec<SuccessTestResult>,
}

impl TestResults {
    /// Checks that results of [Mocha] tests has error.
    ///
    /// Returns `true` if failures vector not empty.
    ///
    /// [Mocha]: https://mochajs.org/
    pub fn is_has_error(&self) -> bool {
        !self.failures.is_empty()
    }
}

impl fmt::Display for TestResults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\nPassed tests ({}):\n\n", self.stats.passes)?;
        for passed in &self.passes {
            writeln!(f, "{}", passed)?;
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
        write!(f, "failures: {}; ", self.stats.failures)?;
        writeln!(f, "total duration: {}ms.", self.stats.duration)?;

        Ok(())
    }
}

/// Summary stats of [Mocha] tests.
///
/// [Mocha]: https://mochajs.org/
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestStats {
    /// Count of tested suites.
    pub suites: i32,

    /// Count of tested tests.
    pub tests: i32,

    /// Count of passed tests.
    pub passes: i32,

    /// Count of pending tests.
    ///
    /// This is __useless__ field because in our case this count always zero.
    pub pending: i32,

    /// Count of failed tests.
    pub failures: i32,

    /// Time when tests started.
    pub start: String,

    /// Time when tests completed.
    pub end: String,

    /// Total duration of all tests.
    pub duration: u32,
}

/// Error of failed test.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestError {
    /// Error message.
    pub message: Option<String>,

    /// Stacktrace from JS side where exception thrown.
    pub stack: String,
}

/// Test which successfully passed.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuccessTestResult {
    /// Title of test.
    pub title: String,

    /// Title of test with context.
    pub full_title: String,

    /// Duration of test.
    pub duration: u32,
}

impl fmt::Display for SuccessTestResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "   {}",
            Paint::green(format!(
                "test {} ... ok ({}ms)",
                self.full_title, self.duration
            ))
        )?;
        Ok(())
    }
}

/// Failed test.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailureTestResult {
    /// Title of test.
    pub title: String,

    /// Title of test with context.
    pub full_title: String,

    /// How much retries happened.
    pub current_retry: i32,

    /// What error happened.
    pub err: TestError,
}

impl fmt::Display for FailureTestResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "   {}\n\n",
            Paint::red(format!(
                "test {} ... failed ({} retry)",
                self.full_title, self.current_retry
            ))
        )?;
        if let Some(err_message) = &self.err.message {
            write!(f, "   Message: {}", err_message)?;
        }
        write!(f, "\n   Stacktrace:\n\n   {}\n\n", self.err.stack)?;
        Ok(())
    }
}
