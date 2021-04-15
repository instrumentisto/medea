//! `wasm32`-specific utility structs and functions.

mod callback;
mod event_listener;

#[doc(inline)]
pub use self::{
    callback::{Callback, Function},
    event_listener::{EventListener, EventListenerBindError},
};
