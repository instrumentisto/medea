//! Widely used types and functions

pub use std::{
    boxed::Box,
    sync::{Arc, Mutex, MutexGuard},
};

pub use core::pin::Pin;

pub use core::{
    future::Future,
    mem::transmute,
    ptr::null_mut,
    task::{Context, Poll},
};

pub use super::woke::{waker_ref, Woke as Wake};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

pub fn mutex_lock<T>(mutex: &Mutex<T>) -> MutexGuard<T> {
    {
        mutex.lock().unwrap()
    }
}
