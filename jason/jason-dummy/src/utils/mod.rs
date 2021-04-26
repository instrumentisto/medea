mod arrays;
mod closure;
mod completer;
mod dart_api;
mod string;

use std::future::Future;

use dart_sys::Dart_Handle;

pub use self::{
    arrays::PtrArray,
    closure::DartClosure,
    completer::Completer,
    string::{c_str_into_string, string_into_c_str},
};

/// Spawns provided [`Future`] in the Dart event loop.
pub fn spawn<F>(_: F)
where
    F: Future<Output = ()> + 'static,
{
    todo!()
}

/// Converts provided [`Future`] to the Dart `Future`.
///
/// Returns [`Dart_Handle`] to the created Dart `Future`.
pub fn into_dart_future<F, O, E>(f: F) -> Dart_Handle
where
    F: Future<Output = Result<O, E>> + 'static,
    O: 'static,
    E: 'static,
{
    let completer = Completer::new();
    let fut = completer.future();
    spawn(async move {
        match f.await {
            Ok(o) => {
                completer.complete(o);
            }
            Err(e) => {
                completer.complete_error(e);
            }
        }
    });
    fut
}
