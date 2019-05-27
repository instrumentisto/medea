#[macro_use]
extern crate quote;
extern crate proc_macro;

mod state_machine_shared_fn_accessor;
mod event_dispatcher;

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

/// This is macro for generating Handler trait based on
/// some enum with named fields.
/// ## Derive macro use
/// ```
/// use medea_derives::EventDispatcher;
///
/// #[derive(EventDispatcher)]
/// enum Event {
///     SdpAnswerMade {
///         peer_id: u64, sdp_answer: String
///     },
/// }
///
/// struct Room {
///     // ...
/// }
///
/// // The principle of generation Handler trait name
/// // is to simply add postfix "Handler".
/// // Example:
/// // Original enum name is Event then handler name
/// // for this enum will be "EventHandler".
/// impl EventHandler for Room {
///     // The name of the function is generated based
///     // on the name of the enumeration [`Event`].
///     // The principle of its generation
///     // is to translate the name from camelCase
///     // to snake_case and add the prefix "on".
///     // Example:
///     // Original enum variant name is SomeEnumVariant then handler name
///     // for this variant will be on_some_enum_variant.
///     fn on_sdp_answer_made(&mut self, peer_id: u64, sdp_answer: String) {
///         // Some handler code
///     }
/// }
///
/// // A function that accepts an [`Event`]
/// // and must pass it to the desired function.
/// fn some_function(event: Event, room: &mut Room) {
///     // This function will call the necessary function
///     // based on the variant of enum [`Event`]
///     event.dispatch(room);
/// }
/// ```
#[proc_macro_derive(EventDispatcher)]
pub fn derive_event_dispatcher(
    input: TokenStream,
) -> TokenStream {
    event_dispatcher::derive(input)
}
