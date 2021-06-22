#![allow(clippy::module_name_repetitions)]
#![forbid(non_ascii_idents, unsafe_code)]

mod callbacks;
mod grpc_control_api;
pub mod signalling;

/// Polls `$name` [`Stream`] until finds provided `$pattern`.
///
/// When provided `$pattern` found - executes provided `$body`.
///
/// This macro can be used only in the `async` blocks.
///
/// # Usage
///
/// ```ignore
/// enum Foo {
///     A(u32),
///     B(u32),
/// }
///
/// let (tx, mut rx) = mpsc::unbounded();
/// tx.unbounded_send(Foo::A(1));
/// tx.unbounded_send(Foo::B(1));
/// tx.unbounded_send(Foo::B(2));
///
/// if_let_next! {
///     Foo::B(i) = rx {
///         assert_eq!(i, 1);
///     }
/// }
/// assert_eq!(rx.next().await.unwrap(), Foo::B(2));
/// ```
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
