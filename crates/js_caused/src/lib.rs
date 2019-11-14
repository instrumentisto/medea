//! Error-handling library for `medea-jason` crate.

use std::fmt::{Debug, Display};

pub use js_caused_derive::*;

/// Representation of an error which can caused by error returned from the
/// JS side.
pub trait JsCaused: Display + Debug + Send + Sync + 'static {
    /// Returns name of error.
    fn name(&self) -> &'static str;

    /// Returns JS error if it is the cause.
    fn js_cause(&self) -> Option<js_sys::Error>;
}
