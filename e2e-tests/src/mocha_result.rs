use serde::Deserialize;
use std::fmt;
use yansi::Paint;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResults {
    pub stats: TestStats,
    pub failures: Vec<FailureTestResult>,
    pub passes: Vec<SuccessTestResult>,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestStats {
    pub suites: i32,
    pub tests: i32,
    pub passes: i32,
    pub pending: i32,
    pub failures: i32,
    pub start: String,
    pub end: String,
    pub duration: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestError {
    pub message: String,
    pub stack: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuccessTestResult {
    pub title: String,
    pub full_title: String,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailureTestResult {
    pub title: String,
    pub full_title: String,
    pub current_retry: i32,
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
        write!(f, "   Message: {}", self.err.message)?;
        write!(f, "\n   Stacktrace:\n\n   {}\n\n", self.err.stack)?;
        Ok(())
    }
}
