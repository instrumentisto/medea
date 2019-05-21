extern crate proc_macro;

mod enum_delegate;

use proc_macro::TokenStream;

/// This macro should be used for creating shared state function's accessor.
/// ## How to use:
///
/// ```
/// #[enum_delegate(pub fn some_value(&self) -> i32)]
/// #[enum_delegate(
///     pub fn function_with_additional_args(&self, some_arg: i32) -> i32
/// )]
/// #[enum_delegate(pub fn mutable_function(&mut self) -> i32)]
/// enum SomeStateMachine {
///     // ...
/// }
/// ```
///
/// ## Example:
/// ```
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
///     peer.some_value(); // -> 10
///
///     peer.function_with_additional_args(100); // -> 100
///
///     peer.mutable_function(); // -> 10
///     peer.some_value(); // -> 1000
/// }
/// ```
#[proc_macro_attribute]
pub fn enum_delegate(args: TokenStream, input: TokenStream) -> TokenStream {
    enum_delegate::derive(args, input)
}
