use derive_more::Display;

#[derive(Clone, Display, Debug, Eq, PartialEq, Hash)]
pub struct GrpcCallbackUrl(String);

#[derive(Clone, Display, Debug, Eq, PartialEq, Hash)]
pub enum CallbackUrl {
    Grpc(GrpcCallbackUrl),
}
