//! Helpers for application errors.

use std::{fmt::Debug, rc::Rc};

use derive_more::{Display, From};

pub use medea_macro::JsCaused;

/// Representation of an error which can caused by error returned from the
/// JS side.
pub trait JsCaused {
    /// Type of wrapper for JS error.
    type Error;

    /// Returns name of error.
    fn name(&self) -> &'static str;

    /// Returns JS error if it is the cause.
    fn js_cause(self) -> Option<Self::Error>;
}

/// Wrapper for [`serde_json::error::Error`] that provides [`Clone`], [`Debug`],
/// [`Display`] implementations.
#[derive(Clone, Debug, Display, From)]
#[from(forward)]
pub struct JsonParseError(Rc<serde_json::error::Error>);

impl PartialEq for JsonParseError {
    fn eq(&self, other: &Self) -> bool {
        self.0.line() == other.0.line()
            && self.0.column() == other.0.column()
            && self.0.classify() == other.0.classify()
    }
}
