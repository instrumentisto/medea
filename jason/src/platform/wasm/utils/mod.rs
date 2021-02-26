mod callback;
mod event_listener;

#[doc(inline)]
pub use self::{
    callback::{Callback, Function},
    event_listener::{EventListener, EventListenerBindError},
};
