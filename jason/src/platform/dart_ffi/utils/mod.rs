//! `wasm32`-specific utility structs and functions.

mod callback;
pub mod dart_api;
mod event_listener;

#[doc(inline)]
pub use self::{
    callback::{Callback, Function},
    event_listener::{EventListener, EventListenerBindError},
};
