//! Macros for [Medea] media server project.
//!
//! This crate is indented for inner use only by [Medea] media server.
//!
//! [Medea]: https://github.com/instrumentisto/medea

extern crate proc_macro;

mod dispatchable;
mod enum_delegate;

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
#[proc_macro_attribute]
pub fn dispatchable(_: TokenStream, input: TokenStream) -> TokenStream {
    dispatchable::derive(input).unwrap_or_else(|e| e.to_compile_error().into())
}
