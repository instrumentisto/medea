mod arrays;
mod string;

use std::future::Future;

use dart_sys::Dart_Handle;

use crate::{
    api::DartValue,
    platform::{spawn, utils::Completer},
};

pub use self::{
    arrays::PtrArray,
    string::{c_str_into_string, string_into_c_str},
};

/// Extension trait for a [`Future`] allowing to convert Rust [`Future`]s to
/// Dart `Future`s.
pub trait IntoDartFuture {
    /// Converts this [`Future`] into a Dart `Future`.
    ///
    /// Returns a [`Dart_Handle`] to the created Dart `Future`.
    ///
    /// __Note, that the Dart `Future` execution begins immediately and cannot
    /// be canceled.__
    fn into_dart_future(self) -> Dart_Handle;
}

impl<F, T, E> IntoDartFuture for F
where
    F: Future<Output = Result<T, E>> + 'static,
    T: Into<DartValue> + 'static,
    E: Into<DartValue> + 'static,
{
    fn into_dart_future(self) -> Dart_Handle {
        let completer = Completer::new();
        let dart_future = completer.future();
        spawn(async move {
            match self.await {
                Ok(ok) => {
                    completer.complete(ok);
                }
                Err(e) => {
                    completer.complete_error(e);
                }
            }
        });
        dart_future
    }
}
