//! Macros for [Medea] media server project.
//!
//! This crate is indented for inner use only by [Medea] media server.
//!
//! [Medea]: https://github.com/instrumentisto/medea

extern crate proc_macro;

mod enum_delegate;
mod event_dispatcher;

use proc_macro::TokenStream;

/// Delegates function calls to enum variants field.
/// Variants are expected to have only one field.
///
/// # How to use
///
/// ```
/// use medea_macro::enum_delegate;
///
/// #[enum_delegate(pub fn as_str(&self) -> &str)]
/// #[enum_delegate(pub fn push_str(&mut self, arg: &str))]
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
///
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
#[allow(clippy::needless_pass_by_value)]
#[proc_macro_attribute]
pub fn enum_delegate(args: TokenStream, input: TokenStream) -> TokenStream {
    enum_delegate::derive(&args, input)
        .unwrap_or_else(|e| e.to_compile_error().into())
}

/// This is macro for generating Handler trait based on
/// some enum with named fields.
/// ## Derive macro use
/// ```
/// use medea_macro::EventDispatcher;
///
/// #[derive(EventDispatcher)]
/// enum Event {
///     SomeEvent { new_bar: i32 },
///     AnotherEvent,
/// }
///
/// struct Foo {
///     bar: i32,
/// }
///
/// // The principle of generation Handler trait name
/// // is to simply add postfix "Handler".
/// // Example:
/// // Original enum name is Event then handler name
/// // for this enum will be "EventHandler".
/// impl EventHandler for Foo {
///     // The name of the function is generated based
///     // on the name of the enumeration [`Event`].
///     // The principle of its generation
///     // is to translate the name from camelCase
///     // to snake_case and add the prefix "on".
///     // Example:
///     // Original enum variant name is SomeEnumVariant then handler name
///     // for this variant will be on_some_enum_variant.
///     fn on_some_event(&mut self, new_bar: i32) {
///         self.bar = new_bar;
///     }
///
///     fn on_another_event(&mut self) {
///         self.bar = 2;
///     }
/// }
///
/// fn main() {
///     let mut foo = Foo { bar: 0 };
///
///     Event::SomeEvent { new_bar: 1 }.dispatch(&mut foo);
///     assert_eq!(foo.bar, 1);
///
///     Event::AnotherEvent.dispatch(&mut foo);
///     assert_eq!(foo.bar, 2);
/// }
/// ```
#[proc_macro_derive(EventDispatcher)]
pub fn derive_event_dispatcher(input: TokenStream) -> TokenStream {
    event_dispatcher::derive(input)
}
