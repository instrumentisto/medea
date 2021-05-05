mod arrays;
mod result;
mod string;

use std::future::Future;

use dart_sys::Dart_Handle;
use futures::FutureExt as _;

use crate::{
    api::{dart::JasonError, DartValue},
    platform::utils::Completer,
};

pub use self::{
    arrays::PtrArray,
    result::DartResult,
    string::{c_str_into_string, string_into_c_str},
};

/// Spawns provided [`Future`] in the Dart event loop.
pub fn spawn<F>(fut: F)
where
    F: Future<Output = ()> + 'static,
{
    fut.now_or_never().unwrap();
}

/// Converts provided [`Future`] to the Dart `Future`.
///
/// Returns [`Dart_Handle`] to the created Dart `Future`.
pub fn future_to_dart<F, T>(f: F) -> Dart_Handle
where
    F: Future<Output = Result<T, JasonError>> + 'static,
    T: Into<DartValue> + 'static,
{
    let completer = Completer::new();
    let dart_future = completer.future();
    spawn(async move {
        match f.await {
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
