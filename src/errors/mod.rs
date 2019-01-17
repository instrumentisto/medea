use failure::Fail;

#[derive(Fail, Debug)]
pub enum AppError {
    #[fail(display = "Not found member")]
    NotFound,
}
