use derive_more::Display;

#[derive(Clone, Display, Eq, PartialEq, Hash)]
pub struct GrpcCallbackUrl(String);

#[derive(Clone, Display, Eq, PartialEq, Hash)]
pub enum CallbackUrl {
    Grpc(GrpcCallbackUrl),
}
