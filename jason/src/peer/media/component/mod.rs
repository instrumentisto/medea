//! Implementation of the [`SenderComponent`] and [`ReceiverComponent`].

mod receiver;
mod sender;

#[doc(inline)]
pub use self::{
    receiver::{ReceiverComponent, ReceiverState},
    sender::{SenderComponent, SenderState},
};
