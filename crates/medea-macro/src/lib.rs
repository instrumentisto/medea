//! Macros for [Medea] media server project.
//!
//! This crate is indented for inner use only by [Medea] media server.
//!
//! [Medea]: https://github.com/instrumentisto/medea

mod dispatchable;
mod enum_delegate;
mod js_caused;

use proc_macro::TokenStream;
use synstructure::decl_derive;

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
/// let mut foo = MyEnum::Foo(String::from("foo"));
/// foo.push_str("_bar");
/// assert_eq!(foo.as_str(), "foo_bar")
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
/// let mut peer = PeerStateMachine::SomeState(Peer {
///     context: Context { some_value: 10 },
///     state: SomeState,
/// });
///
/// assert_eq!(peer.some_value(), 10);
///
/// assert_eq!(peer.function_with_additional_args(100), 100);
///
/// assert_eq!(peer.mutable_function(), 10);
/// assert_eq!(peer.some_value(), 1000);
/// ```
#[allow(clippy::needless_pass_by_value)]
#[proc_macro_attribute]
pub fn enum_delegate(args: TokenStream, input: TokenStream) -> TokenStream {
    enum_delegate::derive(&args, input)
        .unwrap_or_else(|e| e.to_compile_error().into())
}

/// Generates `*Handler` trait and displatching function for some event,
/// represented as `enum`.
///
/// # How to use
///
/// ### 1. Declare `enum` for event variants and a `struct` to handle them.
/// ```
/// use medea_macro::dispatchable;
///
/// #[dispatchable]
/// enum Event {
///     Some { new_bar: i32 },
///     Another,
///     UnnamedVariant(i32, i32),
/// }
///
/// struct Foo {
///     bar: i32,
///     baz: i32,
/// }
/// ```
///
/// ### 2. Implement handler for your `struct`.
///
/// For the given `enum` macro generates a unique trait by adding `Handler`
/// to the end of its name. Each method of trait is created by `snake_case`'ing
/// `enum` variants and adding `on_` prefix.
///
/// `type Output` is a type which will be returned from all functions of
/// `EventHandler` trait.
///
/// ```
/// # use medea_macro::dispatchable;
/// #
/// # #[dispatchable]
/// # enum Event {
/// #     Some { new_bar: i32 },
/// #     Another,
/// #     UnnamedVariant(i32, i32),
/// # }
/// #
/// # struct Foo {
/// #     bar: i32,
/// #     baz: i32,
/// # }
/// #
/// impl EventHandler for Foo {
///     type Output = i32;
///
///     fn on_some(&mut self, new_bar: i32) -> Self::Output {
///         self.bar = new_bar;
///         self.bar
///     }
///
///     fn on_another(&mut self) -> Self::Output {
///         self.bar = 2;
///         self.bar
///     }
///
///     fn on_unnamed_variant(&mut self, data: (i32, i32)) -> Self::Output {
///         self.bar = data.0;
///         self.baz = data.1;
///         self.bar
///     }
/// }
/// ```
///
/// ### 3. Dispatch event with handler
///
/// For the given `enum` macro generates `dispatch_with()` method to dispatch
/// `enum` with a given handler.
///
/// ```
/// # use medea_macro::dispatchable;
/// #
/// # #[dispatchable]
/// # enum Event {
/// #     Some { new_bar: i32 },
/// #     Another,
/// #     UnnamedVariant(i32, i32),
/// # }
/// #
/// # struct Foo {
/// #     bar: i32,
/// #     baz: i32,
/// # }
/// #
/// # impl EventHandler for Foo {
/// #    type Output = i32;
/// #
/// #    fn on_some(&mut self, new_bar: i32) -> Self::Output {
/// #        self.bar = new_bar;
/// #        self.bar
/// #    }
/// #
/// #    fn on_another(&mut self) -> Self::Output {
/// #        self.bar = 2;
/// #        self.bar
/// #    }
/// #
/// #    fn on_unnamed_variant(&mut self, data: (i32, i32)) -> Self::Output {
/// #        self.bar = data.0;
/// #        self.baz = data.1;
/// #        self.bar
/// #    }
/// # }
/// #
///
/// let mut foo = Foo { bar: 0, baz: 0 };
///
/// let bar = Event::Some { new_bar: 1 }.dispatch_with(&mut foo);
/// assert_eq!(foo.bar, 1);
/// assert_eq!(bar, 1);
///
/// let bar = Event::Another.dispatch_with(&mut foo);
/// assert_eq!(foo.bar, 2);
/// assert_eq!(bar, 2);
///
/// let bar = Event::UnnamedVariant(3, 3).dispatch_with(&mut foo);
/// assert_eq!(foo.bar, 3);
/// assert_eq!(foo.baz, 3);
/// assert_eq!(bar, 3);
/// ```
/// ### Optional. You can change `self` type in handler functions.
///
/// All handler functions are take mutable reference to `Self`, you can spevify
/// type manually if default does not suite your needs.
///
/// ```
/// # use std::rc::Rc;
/// # use medea_macro::dispatchable;
/// #
/// # #[dispatchable(Rc<Self>)]
/// # enum Event {
/// #     Variant,
/// # }
/// #
/// # struct Foo;
/// #
/// # impl EventHandler for Foo {
/// #    type Output = ();
/// #
/// #    fn on_variant(self: Rc<Self>) {
/// #    }
/// # }
/// #
///
/// let foo = Rc::new(Foo);
///
/// Event::Variant.dispatch_with(foo);
/// ```
#[proc_macro_attribute]
pub fn dispatchable(args: TokenStream, input: TokenStream) -> TokenStream {
    let enum_item = syn::parse_macro_input!(input as dispatchable::Item);
    let args = syn::parse_macro_input!(args as dispatchable::Args);
    dispatchable::expand(enum_item, &args)
}

decl_derive!([JsCaused, attributes(js)] =>
/// Generate implementation of `JsCaused` trait for errors represented as enum.
///
/// # How to use
///
/// ### 1. Declare wrapper for JS error and enum for error variants.
///
/// The `js_cause()` method returns error if nested error has its type declared
/// as an argument of the attribute `#[js(error = "path::to::Error")]` or
/// the error type is assumed to be imported as `JsError`.
///
/// ```
/// use medea_jason::utils::JsCaused;
///
/// struct JsError;
///
/// #[derive(JsCaused)]
/// enum FooError {
///     Internal,
///     Js(JsError),
/// }
///
/// let err = FooError::Internal;
/// assert_eq!(err.name(), "Internal");
/// assert!(err.js_cause().is_none());
///
/// let err = FooError::Js(JsError {});
/// assert_eq!(err.name(), "Js");
/// assert!(err.js_cause().is_some());
/// ```
///
/// If enum variant has attribute `#[js(cause)]` it will call the `js_cause()`
/// method on nested error.
///
/// ```
/// # use medea_jason::utils::JsCaused;
/// #
/// # struct JsError;
/// #
/// # #[derive(JsCaused)]
/// # enum FooError {
/// #     Internal,
/// #     Js(JsError),
/// # }
/// #
/// #[derive(JsCaused)]
/// enum BarError {
///     Foo(#[js(cause)] FooError),
/// }
///
/// let err = BarError::Foo(FooError::Internal);
/// assert_eq!(err.name(), "Foo");
/// assert!(err.js_cause().is_none());
///
/// let err = BarError::Foo(FooError::Js(JsError {}));
/// assert_eq!(err.name(), "Foo");
/// assert!(err.js_cause().is_some());
/// ```
js_caused::derive);
