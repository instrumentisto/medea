//! Facilities for creating Dart exceptions from Rust.

use std::{borrow::Cow, ptr};

use dart_sys::Dart_Handle;
use derive_more::Into;
use libc::c_char;
use tracerr::Trace;

use crate::{
    api::dart::{utils::string_into_c_str, DartValue},
    platform,
};

/// Pointer to an extern function that returns a new Dart [`ArgumentError`] with
/// the provided invalid argument, its `name` and error `message` describing the
/// problem.
///
/// [`ArgumentError`]: https://api.dart.dev/dart-core/ArgumentError-class.html
type NewArgumentErrorCaller = extern "C" fn(
    value: DartValue,
    name: ptr::NonNull<c_char>,
    message: ptr::NonNull<c_char>,
) -> Dart_Handle;

/// Pointer to an extern function that returns a new Dart [`StateError`] with
/// the provided error message.
///
/// [`StateError`]: https://api.dart.dev/dart-core/StateError-class.html
type NewStateErrorCaller = extern "C" fn(ptr::NonNull<c_char>) -> Dart_Handle;

/// Pointer to an extern function that returns a new Dart
/// `MediaManagerException` with the provided `kind`, `message`, `cause` and
/// `stacktrace`.
type NewMediaManagerExceptionCaller = extern "C" fn(
    kind: MediaManagerExceptionKind,
    message: ptr::NonNull<c_char>,
    cause: DartValue,
    stacktrace: ptr::NonNull<c_char>,
) -> Dart_Handle;

/// Stores pointer to the [`NewArgumentErrorCaller`] extern function.
///
/// Must be initialized by Dart during FFI initialization phase.
static mut NEW_ARGUMENT_ERROR_CALLER: Option<NewArgumentErrorCaller> = None;

/// Stores pointer to the [`NewStateErrorCaller`] extern function.
///
/// Must be initialized by Dart during FFI initialization phase.
static mut NEW_STATE_ERROR_CALLER: Option<NewStateErrorCaller> = None;

/// Stores pointer to the [`NewMediaManagerExceptionCaller`] extern function.
///
/// Must be initialized by Dart during FFI initialization phase.
static mut NEW_MEDIA_MANAGER_EXCEPTION_CALLER: Option<
    NewMediaManagerExceptionCaller,
> = None;

/// Registers the provided [`NewArgumentErrorCaller`] as
/// [`NEW_ARGUMENT_ERROR_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_new_argument_error_caller(
    f: NewArgumentErrorCaller,
) {
    NEW_ARGUMENT_ERROR_CALLER = Some(f);
}

/// Registers the provided [`NewStateErrorCaller`] as
/// [`NEW_STATE_ERROR_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_new_state_error_caller(
    f: NewStateErrorCaller,
) {
    NEW_STATE_ERROR_CALLER = Some(f);
}

/// Registers the provided [`NewMediaManagerExceptionCaller`] as
/// [`NEW_MEDIA_MANAGER_EXCEPTION_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_new_media_manager_exception_caller(
    f: NewMediaManagerExceptionCaller,
) {
    NEW_MEDIA_MANAGER_EXCEPTION_CALLER = Some(f);
}

/// An error that can be returned from Rust to Dart.
#[derive(Into)]
#[repr(transparent)]
pub struct DartError(ptr::NonNull<Dart_Handle>);

impl DartError {
    /// Creates a new [`DartError`] from the provided [`Dart_Handle`].
    #[inline]
    #[must_use]
    fn new(handle: Dart_Handle) -> DartError {
        DartError(ptr::NonNull::from(Box::leak(Box::new(handle))))
    }
}

impl From<platform::Error> for DartError {
    fn from(err: platform::Error) -> Self {
        Self::new(err.get_handle())
    }
}

/// Error returning by Rust when an unacceptable argument is passed to a
/// function through FFI.
///
/// It can be converted into a [`DartError`] and passed to Dart.
pub struct ArgumentError<T> {
    /// Invalid value of the argument.
    val: T,

    /// Name of the invalid argument.
    name: &'static str,

    /// Message describing the problem.
    message: Cow<'static, str>,
}

impl<T> ArgumentError<T> {
    /// Creates a new [`ArgumentError`] from the provided invalid argument, its
    /// `name` and error `message` describing the problem.
    #[inline]
    #[must_use]
    pub fn new<V>(val: T, name: &'static str, message: V) -> Self
    where
        V: Into<Cow<'static, str>>,
    {
        Self {
            val,
            name,
            message: message.into(),
        }
    }
}

impl<T: Into<DartValue>> From<ArgumentError<T>> for DartError {
    #[inline]
    fn from(err: ArgumentError<T>) -> Self {
        unsafe {
            Self::new(NEW_ARGUMENT_ERROR_CALLER.unwrap()(
                err.val.into(),
                string_into_c_str(err.name.to_owned()),
                string_into_c_str(err.message.into_owned()),
            ))
        }
    }
}

/// Error that is thrown when the operation was not allowed by the current state
/// of the object.
///
/// It can be converted into a [`DartError`] and passed to Dart.
pub struct StateError(Cow<'static, str>);

impl StateError {
    /// Creates a new [`StateError`] with the provided error `message`
    /// describing the problem.
    pub fn new<T: Into<Cow<'static, str>>>(message: T) -> Self {
        Self(message.into())
    }
}

impl From<StateError> for DartError {
    #[inline]
    fn from(err: StateError) -> Self {
        unsafe {
            Self::new(NEW_STATE_ERROR_CALLER.unwrap()(string_into_c_str(
                err.0.into_owned(),
            )))
        }
    }
}

/// Concrete error kind of a [`MediaManagerException`].
#[repr(u8)]
pub enum MediaManagerExceptionKind {
    /// Occurs if the [getUserMedia][1] request failed.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    GetUserMediaFailed,

    /// Occurs if the [getDisplayMedia()][1] request failed.
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    GetDisplayMediaFailed,

    /// Occurs when cannot get info about connected [MediaDevices][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams#mediadevices
    EnumerateDevicesFailed,

    /// Occurs when local track is [`ended`][1] right after [getUserMedia()][2]
    /// or [getDisplayMedia()][3] request.
    ///
    /// [1]: https://tinyurl.com/w3-streams#idl-def-MediaStreamTrackState.ended
    /// [2]: https://tinyurl.com/rnxcavf
    /// [3]: https://w3.org/TR/screen-capture#dom-mediadevices-getdisplaymedia
    LocalTrackIsEnded,
}

/// Exception that can be thrown when accessing media devices.
pub struct MediaManagerException {
    /// Concrete error kind of this [`MediaManagerException`].
    kind: MediaManagerExceptionKind,

    /// Error message describing the problem.
    message: Cow<'static, str>,

    /// [`platform::Error`] that caused this [`MediaManagerException`].
    cause: Option<platform::Error>,

    /// Stacktrace of this [`MediaManagerException`].
    trace: Trace,
}

impl MediaManagerException {
    /// Creates a new [`MediaManagerException`] from the provided `kind`, error
    /// `message`, optional `cause` and `trace`.
    #[inline]
    #[must_use]
    pub fn new<M: Into<Cow<'static, str>>>(
        kind: MediaManagerExceptionKind,
        message: M,
        cause: Option<platform::Error>,
        trace: Trace,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            cause,
            trace,
        }
    }
}

impl From<MediaManagerException> for DartError {
    #[inline]
    fn from(err: MediaManagerException) -> Self {
        unsafe {
            Self::new(NEW_MEDIA_MANAGER_EXCEPTION_CALLER.unwrap()(
                err.kind,
                string_into_c_str(err.message.into_owned()),
                err.cause.map(DartError::from).into(),
                string_into_c_str(err.trace.to_string()),
            ))
        }
    }
}
