//! Macros for [Medea] media server project.
//!
//! This crate is indented for inner use only by [Medea] media server.
//!
//! [Medea]: https://github.com/instrumentisto/medea

extern crate proc_macro;

mod enum_delegate;

use proc_macro::TokenStream;

/// Delegates functions to enum variants field. Variants are expected to have
/// only one field.
///
/// # How to use
///
/// ```
/// use medea_macro::enum_delegate;
///
/// #[enum_delegate(pub fn as_str(&self) -> &str)]
/// #[enum_delegate(pub fn push_str(&mut self, arg:&str))]
/// enum MyEnum {
///     Foo(String),
///     Bar(String),
/// }
///
/// fn main() {
///     let mut foo = MyEnum::Foo(String::from("foo"));
///     foo.push_str("_bar");
///     assert_eq!(foo.as_str(), "foo_bar")
/// }
/// ```
///
/// # Extended example
///
/// ```
/// use medea_macro::enum_delegate;
/// struct SomeState;
/// struct AnotherState;
///
/// struct Context {
///     some_value: i32,
/// }
///
/// struct Peer<S> {
///     context: Context,
///     state: S,
/// }
///
/// impl<T> Peer<T> {
///     pub fn some_value(&self) -> i32 {
///         self.context.some_value
///     }
///
///     pub fn function_with_additional_args(&self, some_arg: i32) -> i32 {
///         some_arg
///     }
///
///     pub fn mutable_function(&mut self) -> i32 {
///         let old_value = self.context.some_value;
///         self.context.some_value = 1000;
///         old_value
///     }
/// }
///
/// #[enum_delegate(pub fn some_value(&self) -> i32)]
/// #[enum_delegate(
///     pub fn function_with_additional_args(&self, some_arg: i32) -> i32
/// )]
/// #[enum_delegate(pub fn mutable_function(&mut self) -> i32)]
/// enum PeerStateMachine {
///     SomeState(Peer<SomeState>),
///     AnotherState(Peer<AnotherState>),
/// }
///
/// fn main() {
///     let mut peer = PeerStateMachine::SomeState(Peer {
///         context: Context { some_value: 10 },
///         state: SomeState,
///     });
///
///     assert_eq!(peer.some_value(), 10);
///
///     assert_eq!(peer.function_with_additional_args(100), 100);
///
///     assert_eq!(peer.mutable_function(), 10);
///     assert_eq!(peer.some_value(), 1000);
/// }
/// ```
#[proc_macro_attribute]
#[allow(clippy::needless_pass_by_value)]
pub fn enum_delegate(args: TokenStream, input: TokenStream) -> TokenStream {
    enum_delegate::derive(&args, input)
}
