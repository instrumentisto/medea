//! Multiplatform Dart runtime specific utility structs and functions.

mod callback;
pub mod completer;
pub mod dart_api;
mod event_listener;

#[doc(inline)]
pub use self::{
    callback::{Callback, Function},
    completer::Completer,
    event_listener::{EventListener, EventListenerBindError},
};
