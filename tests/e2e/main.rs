#![allow(clippy::module_name_repetitions)]

mod callbacks;
mod grpc_control_api;
pub mod signalling;

/// Equality comparisons for the enum variants.
///
/// This macro will ignore all content of the enum, it just compare
/// enum variants not they data.
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

/// Expands to the [`module_path`] + '::' + `function_name`.
#[macro_export]
macro_rules! test_name {
    () => {
        concat!(module_path!(), "::", function_name!())
            .replace("::", "__")
            .as_str()
    };
}
