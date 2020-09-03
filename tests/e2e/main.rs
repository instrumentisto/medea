#![allow(clippy::module_name_repetitions)]

mod callbacks;
mod grpc_control_api;
pub mod signalling;

#[macro_export]
macro_rules! if_let_next {
    ($pattern:pat = $name:ident $body:block ) => {
        loop {
            if let $pattern = $name.select_next_some().await {
                $body;
                break;
            }
        }
    };
}

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

/// Expands to the [`module_path`] and function name, but `::` replaced with
/// `__`.
///
/// Can be used only with [`function_name::named`] macro.
///
/// # Example
///
/// ```
/// use function_name::named;
///
/// use crate::test_name;
///
/// mod foo {
///     mod bar {
///         #[named]
///         fn baz() {
///             assert_eq!(test_name!(), "e2e__foo__bar__baz");
///         }
///     }
/// }
///
/// foo::bar::baz();
/// ```
#[macro_export]
macro_rules! test_name {
    () => {
        concat!(module_path!(), "::", function_name!())
            .replace("::", "__")
            .as_str()
    };
}
