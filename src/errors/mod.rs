use failure::Fail;

use crate::api::control::ControlError;

#[derive(Fail, Debug)]
pub enum AppError {
    #[fail(display = "Not implemented")]
    Control(#[fail(cause)] ControlError),
}
