mod arrays;
mod string;

use std::future::Future;

use dart_sys::Dart_Handle;

use crate::{api::DartValue, platform::utils::Completer};

pub use self::{
    arrays::PtrArray,
    string::{c_str_into_string, string_into_c_str},
};

/// Spawns provided [`Future`] in the Dart event loop.
pub fn spawn<F>(fut: F)
where
    F: Future<Output = ()> + 'static,
{
    crate::platform::spawn(fut);
}

/// Converts provided [`Future`] to the Dart `Future`.
///
/// Returns [`Dart_Handle`] to the created Dart `Future`.
pub fn future_to_dart<F, T, E>(f: F) -> Dart_Handle
where
    F: Future<Output = Result<T, E>> + 'static,
    T: Into<DartValue> + 'static,
    E: Into<DartValue> + 'static,
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
