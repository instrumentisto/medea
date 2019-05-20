extern crate proc_macro;

mod state_machine_shared_fn_accessor;

use proc_macro::TokenStream;

/// This macro should be used for creating shared state function's accessor.
/// ## How to use:
///
/// ```
/// #[state_machine_shared_fn_accessor(
///     /*SHARED_STATE_METHOD*/ -> /*RETURN_TYPE_OF_THIS_METHOD*/
/// )]
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
/// }
///
/// #[state_machine_shared_fn_accessor(some_value -> i32)]
/// enum PeerStateMachine {
///     SomeState(Peer<SomeState>),
///     AnotherState(Peer<AnotherState>),
/// }
///
/// fn main() {
///     let peer = PeerStateMachine::SomeState(Peer {
///         context: Context { some_value: 10 },
///         state: SomeState,
///     });
///
///     peer.some_value() // -> 10
/// }
/// ```
#[proc_macro_attribute]
pub fn state_machine_shared_fn_accessor(
    args: TokenStream,
    input: TokenStream,
) -> TokenStream {
    state_machine_shared_fn_accessor::derive(args, input)
}
