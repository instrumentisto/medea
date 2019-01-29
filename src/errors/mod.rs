use failure::Fail;

use crate::api::control::ControlError;

/// This type of error covers any error that may occur in this application.
#[derive(Fail, Debug)]
pub enum AppError {
    /// This error encompasses any error that can be returned from Control API.
    #[fail(display = "Not implemented")]
    Control(#[fail(cause)] ControlError),
}
