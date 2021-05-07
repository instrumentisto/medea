//! Multiplatform Dart runtime specific utility structs and functions.

pub mod completer;
pub mod dart_api;
mod event_listener;
pub mod function;

#[doc(inline)]
pub use self::{
    completer::Completer,
    event_listener::{EventListener, EventListenerBindError},
    function::Function,
};
