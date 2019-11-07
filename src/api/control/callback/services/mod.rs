//! Implementations of Control API callback services for all protocols.

use grpcio::Error;

pub mod grpc;

#[derive(Debug)]
pub enum CallbackServiceError {
    Grpcio(grpcio::Error),
}

impl From<grpcio::Error> for CallbackServiceError {
    fn from(err: Error) -> Self {
        Self::Grpcio(err)
    }
}
