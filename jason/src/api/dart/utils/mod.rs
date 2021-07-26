mod arrays;
mod err;
mod result;
mod string;

use std::{future::Future, marker::PhantomData};

use dart_sys::Dart_Handle;

use crate::{
    api::DartValue,
    platform::{spawn, utils::Completer},
};

pub use self::{
    arrays::PtrArray,
    err::{ArgumentError, DartError},
    result::DartResult,
    string::{c_str_into_string, string_into_c_str},
};

/// Rust representation of a Dart [`Future`].
///
/// [`Future`]: https://api.dart.dev/dart-async/Future-class.html
#[repr(transparent)]
pub struct DartFuture<O>(Dart_Handle, PhantomData<*const O>);

/// Extension trait for a [`Future`] allowing to convert Rust [`Future`]s to
/// [`DartFuture`]s.
pub trait IntoDartFuture {
    /// The type of the value produced on the [`DartFuture`]'s completion.
    type Output;

    /// Converts this [`Future`] into a Dart `Future`.
    ///
    /// Returns a [`Dart_Handle`] to the created Dart `Future`.
    ///
    /// __Note, that the Dart `Future` execution begins immediately and cannot
    /// be canceled.__
    fn into_dart_future(self) -> DartFuture<Self::Output>;
}

impl<Fut, Ok, Err> IntoDartFuture for Fut
where
    Fut: Future<Output = Result<Ok, Err>> + 'static,
    Ok: Into<DartValue> + 'static,
    Err: Into<DartError>,
{
    type Output = Fut::Output;

    fn into_dart_future(self) -> DartFuture<Fut::Output> {
        let completer = Completer::new();
        let dart_future = completer.future();
        spawn(async move {
            match self.await {
                Ok(ok) => {
                    completer.complete(ok);
                }
                Err(e) => {
                    completer.complete_error(e.into());
                }
            }
        });
        DartFuture(dart_future, PhantomData)
    }
}
