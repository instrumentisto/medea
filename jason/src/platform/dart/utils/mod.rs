//! Multiplatform Dart runtime specific utility structs and functions.

mod callback;
pub mod completer;
pub mod dart_api;
pub mod dart_future;
mod event_listener;

#[doc(inline)]
pub use self::{
    callback::{Callback, Function},
    completer::Completer,
    dart_future::dart_future_to_rust,
    event_listener::{EventListener, EventListenerBindError},
};
