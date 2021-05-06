//! `wasm32`-specific utility structs and functions.

mod event_listener;
mod function;

#[doc(inline)]
pub use self::{
    event_listener::{EventListener, EventListenerBindError},
    function::Function,
};
