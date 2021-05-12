mod arrays;
mod string;

use std::future::Future;

use dart_sys::Dart_Handle;
use futures::FutureExt as _;

use crate::{api::DartValue, platform::utils::Completer};

pub use self::{
    arrays::PtrArray,
    string::{c_str_into_string, string_into_c_str},
};

/// Spawns the provided [`Future`] on the Dart event loop.
pub fn spawn<F>(fut: F)
where
    F: Future<Output = ()> + 'static,
{
    // TODO: Implement executor.
    fut.now_or_never().unwrap();
}

/// Extension trait for the [`Future`] that provides functionality for
/// converting Rust [`Future`]s to the Dart `Future`s.
pub trait IntoDartFuture {
    /// Converts [`Future`] into a Dart `Future`.
    ///
    /// Returns [`Dart_Handle`] to the created Dart `Future`.
    fn into_dart_future(self) -> Dart_Handle;
}

impl<F, T, E> IntoDartFuture for F
where
    F: Future<Output = Result<T, E>> + 'static,
    T: Into<DartValue> + 'static,
    E: Into<DartValue> + 'static,
{
    /// Converts this [`Future`] into a Dart `Future`.
    ///
    /// Returns [`Dart_Handle`] to the created Dart `Future`.
    ///
    /// __Note that the Dart `Future` execution begins immediately and  cannot
    /// be canceled.__
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
