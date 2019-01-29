//! Common errors used in application.

use failure::Fail;

use crate::api::control::ControlError;

/// Any error that may occur in this application.
#[derive(Fail, Debug)]
pub enum AppError {
    /// Error returned from Control API.
    #[fail(display = "Not implemented")]
    Control(#[fail(cause)] ControlError),
}
