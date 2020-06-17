#![allow(clippy::module_name_repetitions)]
mod callbacks;
mod grpc_control_api;
pub mod signalling;

#[macro_export]
macro_rules! enum_eq {
    ($e:path, $val:ident) => {
        if let $e { .. } = $val {
            true
        } else {
            false
        }
    };
}
