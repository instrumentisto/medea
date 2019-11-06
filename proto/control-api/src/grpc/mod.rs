#![allow(
    bare_trait_objects,
    clippy::cargo,
    clippy::nursery,
    clippy::pedantic,
)]

pub mod api;
pub mod api_grpc;
pub mod callback;
pub mod callback_grpc;

mod empty {
    pub use protobuf::well_known_types::Empty;
}