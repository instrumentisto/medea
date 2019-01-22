use crate::api::control::ControlError;
use failure::Fail;

#[derive(Fail, Debug)]
pub enum AppError {
    #[fail(display = "Not implemented")]
    Control(#[fail(cause)] ControlError),
}
