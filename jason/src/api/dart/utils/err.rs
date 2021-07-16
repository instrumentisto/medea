//! Facilities for creating Dart exceptions from Rust.

use std::{borrow::Cow, ptr};

use dart_sys::Dart_Handle;
use derive_more::Into;
use libc::c_char;
use tracerr::{Trace, Traced};

use crate::{
    api::dart::{utils::string_into_c_str, DartValue},
    platform,
    room::ChangeMediaStateError,
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
/// the provided message.
///
/// [`StateError`]: https://api.dart.dev/dart-core/StateError-class.html
type NewStateErrorCaller = extern "C" fn(ptr::NonNull<c_char>) -> Dart_Handle;

/// Pointer to an extern function that returns a new Dart [`FormatException`][1]
/// with the provided message.
///
/// [1]: https://api.dart.dev/dart-core/FormatException-class.html
type NewFormatExceptionCaller =
    extern "C" fn(ptr::NonNull<c_char>) -> Dart_Handle;

/// Pointer to an extern function that returns a new Dart
/// [`LocalMediaInitException`] with the provided error `kind`, `message`,
/// `cause` and `stacktrace`.
type NewLocalMediaInitExceptionCaller = extern "C" fn(
    kind: LocalMediaInitExceptionKind,
    message: ptr::NonNull<c_char>,
    cause: DartValue,
    stacktrace: ptr::NonNull<c_char>,
) -> Dart_Handle;

/// Pointer to an extern function that returns a new Dart
/// [`EnumerateDevicesException`] with the provided error `cause` and
/// `stacktrace`.
type NewEnumerateDevicesExceptionCaller = extern "C" fn(
    cause: DartError,
    stacktrace: ptr::NonNull<c_char>,
) -> Dart_Handle;

/// Pointer to an extern function that returns a new Dart
/// [`RpcClientException`] with the provided error `kind`, `message`,
/// `cause` and `stacktrace`.
type NewRpcClientExceptionCaller = extern "C" fn(
    kind: RpcClientExceptionKind,
    message: ptr::NonNull<c_char>,
    cause: DartValue,
    stacktrace: ptr::NonNull<c_char>,
) -> Dart_Handle;

/// Pointer to an extern function that returns a new Dart
/// [`MediaStateTransitionException`] with the provided error `message` and
/// `stacktrace`.
type NewMediaStateTransitionExceptionCaller = extern "C" fn(
    message: ptr::NonNull<c_char>,
    stacktrace: ptr::NonNull<c_char>,
) -> Dart_Handle;

/// Pointer to an extern function that returns a new Dart [`InternalException`]
/// with the provided error `message`, `cause` and `stacktrace`.
type NewInternalExceptionCaller = extern "C" fn(
    message: ptr::NonNull<c_char>,
    cause: DartValue,
    stacktrace: ptr::NonNull<c_char>,
) -> Dart_Handle;

/// Pointer to an extern function that returns a new Dart
/// [`MediaSettingsUpdateException`] with the provided error `message`, `cause`
/// and `rolled_back` property.
type NewMediaSettingsUpdateExceptionCaller = extern "C" fn(
    message: ptr::NonNull<c_char>,
    cause: DartError,
    rolled_back: u8,
) -> Dart_Handle;

/// Stores pointer to the [`NewArgumentErrorCaller`] extern function.
///
/// Must be initialized by Dart during FFI initialization phase.
static mut NEW_ARGUMENT_ERROR_CALLER: Option<NewArgumentErrorCaller> = None;

/// Stores pointer to the [`NewStateErrorCaller`] extern function.
///
/// Must be initialized by Dart during FFI initialization phase.
static mut NEW_STATE_ERROR_CALLER: Option<NewStateErrorCaller> = None;

/// Stores pointer to the [`NewFormatExceptionCaller`] extern function.
///
/// Must be initialized by Dart during FFI initialization phase.
static mut NEW_FORMAT_EXCEPTION_CALLER: Option<NewFormatExceptionCaller> = None;

/// Stores pointer to the [`NewLocalMediaInitExceptionCaller`] extern function.
///
/// Must be initialized by Dart during FFI initialization phase.
static mut NEW_LOCAL_MEDIA_INIT_EXCEPTION_CALLER: Option<
    NewLocalMediaInitExceptionCaller,
> = None;

/// Stores pointer to the [`NewEnumerateDevicesExceptionCaller`] extern
/// function.
///
/// Must be initialized by Dart during FFI initialization phase.
static mut NEW_ENUMERATE_DEVICES_EXCEPTION_CALLER: Option<
    NewEnumerateDevicesExceptionCaller,
> = None;

/// Stores pointer to the [`NewRpcClientExceptionCaller`] extern function.
///
/// Must be initialized by Dart during FFI initialization phase.
static mut NEW_RPC_CLIENT_EXCEPTION_CALLER: Option<
    NewRpcClientExceptionCaller,
> = None;

/// Stores pointer to the [`NewMediaStateTransitionExceptionCaller`] extern
/// function.
///
/// Must be initialized by Dart during FFI initialization phase.
static mut NEW_MEDIA_STATE_TRANSITION_EXCEPTION_CALLER: Option<
    NewMediaStateTransitionExceptionCaller,
> = None;

/// Stores pointer to the [`NewInternalExceptionCaller`] extern function.
///
/// Must be initialized by Dart during FFI initialization phase.
static mut NEW_INTERNAL_EXCEPTION_CALLER: Option<NewInternalExceptionCaller> =
    None;

/// Stores pointer to the [`NewMediaSettingsUpdateExceptionCaller`] extern
/// function.
///
/// Must be initialized by Dart during FFI initialization phase.
static mut NEW_MEDIA_SETTINGS_UPDATE_EXCEPTION_CALLER: Option<
    NewMediaSettingsUpdateExceptionCaller,
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

/// Registers the provided [`NewFormatExceptionCaller`] as
/// [`NEW_FORMAT_EXCEPTION_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_new_format_exception_caller(
    f: NewStateErrorCaller,
) {
    NEW_FORMAT_EXCEPTION_CALLER = Some(f);
}

/// Registers the provided [`NewLocalMediaInitExceptionCaller`] as
/// [`NEW_LOCAL_MEDIA_INIT_EXCEPTION_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_new_local_media_init_exception_caller(
    f: NewLocalMediaInitExceptionCaller,
) {
    NEW_LOCAL_MEDIA_INIT_EXCEPTION_CALLER = Some(f);
}

/// Registers the provided [`NewLocalMediaInitExceptionCaller`] as
/// [`NEW_ENUMERATE_DEVICES_EXCEPTION_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_new_enumerate_devices_exception_caller(
    f: NewEnumerateDevicesExceptionCaller,
) {
    NEW_ENUMERATE_DEVICES_EXCEPTION_CALLER = Some(f);
}

/// Registers the provided [`NewRpcClientExceptionCaller`] as
/// [`NEW_RPC_CLIENT_EXCEPTION_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_new_rpc_client_exception_caller(
    f: NewRpcClientExceptionCaller,
) {
    NEW_RPC_CLIENT_EXCEPTION_CALLER = Some(f);
}

/// Registers the provided [`NewMediaStateTransitionExceptionCaller`] as
/// [`NEW_MEDIA_STATE_TRANSITION_EXCEPTION_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_new_media_state_transition_exception_caller(
    f: NewMediaStateTransitionExceptionCaller,
) {
    NEW_MEDIA_STATE_TRANSITION_EXCEPTION_CALLER = Some(f);
}

/// Registers the provided [`NewInternalExceptionCaller`] as
/// [`NEW_INTERNAL_EXCEPTION_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_new_internal_exception_caller(
    f: NewInternalExceptionCaller,
) {
    NEW_INTERNAL_EXCEPTION_CALLER = Some(f);
}

/// Registers the provided [`NewMediaSettingsUpdateExceptionCaller`] as
/// [`NEW_MEDIA_SETTINGS_UPDATE_EXCEPTION_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_new_media_settings_update_exception_caller(
    f: NewMediaSettingsUpdateExceptionCaller,
) {
    NEW_MEDIA_SETTINGS_UPDATE_EXCEPTION_CALLER = Some(f);
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
    #[inline]
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

/// Error thrown when the operation wasn't allowed by the current state of the
/// object.
///
/// It can be converted into a [`DartError`] and passed to Dart.
pub struct StateError(Cow<'static, str>);

impl StateError {
    /// Creates a new [`StateError`] with the provided `message` describing the
    /// problem.
    #[inline]
    #[must_use]
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

/// Possible error kinds of a [`LocalMediaInitException`].
#[repr(u8)]
pub enum LocalMediaInitExceptionKind {
    /// Occurs if the [getUserMedia()][1] request failed.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    GetUserMediaFailed,

    /// Occurs if the [getDisplayMedia()][1] request failed.
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    GetDisplayMediaFailed,

    /// Occurs when local track is [`ended`][1] right after [getUserMedia()][2]
    /// or [getDisplayMedia()][3] request.
    ///
    /// [1]: https://tinyurl.com/w3-streams#idl-def-MediaStreamTrackState.ended
    /// [2]: https://tinyurl.com/rnxcavf
    /// [3]: https://w3.org/TR/screen-capture#dom-mediadevices-getdisplaymedia
    LocalTrackIsEnded,
}

/// Exception thrown when accessing media devices.
pub struct LocalMediaInitException {
    /// Concrete error kind of this [`LocalMediaInitException`].
    kind: LocalMediaInitExceptionKind,

    /// Error message describing the problem.
    message: Cow<'static, str>,

    /// [`platform::Error`] that caused this [`LocalMediaInitException`].
    cause: Option<platform::Error>,

    /// Stacktrace of this [`LocalMediaInitException`].
    trace: Trace,
}

impl LocalMediaInitException {
    /// Creates a new [`LocalMediaInitException`] from the provided error
    /// `kind`, `message`, optional `cause` and `trace`.
    #[inline]
    #[must_use]
    pub fn new<M: Into<Cow<'static, str>>>(
        kind: LocalMediaInitExceptionKind,
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

impl From<LocalMediaInitException> for DartError {
    #[inline]
    fn from(err: LocalMediaInitException) -> Self {
        unsafe {
            Self::new(NEW_LOCAL_MEDIA_INIT_EXCEPTION_CALLER.unwrap()(
                err.kind,
                string_into_c_str(err.message.into_owned()),
                err.cause.map(DartError::from).into(),
                string_into_c_str(err.trace.to_string()),
            ))
        }
    }
}

/// Exception thrown when cannot get info of available media devices.
pub struct EnumerateDevicesException {
    /// [`platform::Error`] that caused this [`EnumerateDevicesException`].
    cause: platform::Error,

    /// Stacktrace of this [`EnumerateDevicesException`].
    trace: Trace,
}

impl EnumerateDevicesException {
    /// Creates a new [`EnumerateDevicesException`] from the provided error
    /// `cause` and `trace`.
    #[inline]
    #[must_use]
    pub fn new(cause: platform::Error, trace: Trace) -> Self {
        Self { cause, trace }
    }
}

impl From<EnumerateDevicesException> for DartError {
    #[inline]
    fn from(err: EnumerateDevicesException) -> Self {
        unsafe {
            Self::new(NEW_ENUMERATE_DEVICES_EXCEPTION_CALLER.unwrap()(
                err.cause.into(),
                string_into_c_str(err.trace.to_string()),
            ))
        }
    }
}

/// Exception thrown when a string or some other data doesn't have an expected
/// format and cannot be parsed or processed.
///
/// It can be converted into a [`DartError`] and passed to Dart.
pub struct FormatException(Cow<'static, str>);

impl FormatException {
    /// Creates a new [`FormatException`] with the provided `message` describing
    /// the problem.
    #[inline]
    #[must_use]
    pub fn new<T: Into<Cow<'static, str>>>(message: T) -> Self {
        Self(message.into())
    }
}

impl From<FormatException> for DartError {
    #[inline]
    fn from(err: FormatException) -> Self {
        unsafe {
            Self::new(NEW_FORMAT_EXCEPTION_CALLER.unwrap()(string_into_c_str(
                err.0.into_owned(),
            )))
        }
    }
}

/// Possible error kinds of a [`RpcClientException`].
#[repr(u8)]
pub enum RpcClientExceptionKind {
    /// Connection with a server was lost.
    ///
    /// This usually means that some transport error occurred, so a client can
    /// continue performing reconnecting attempts.
    ConnectionLost,

    /// Could not authorize an RPC session.
    ///
    /// This usually means that authentication data a client provides is
    /// obsolete.
    AuthorizationFailed,

    /// RPC session has been finished. This is a terminal state.
    SessionFinished,
}

/// Exceptions thrown from an RPC client that implements messaging with media
/// server.
pub struct RpcClientException {
    /// Concrete error kind of this [`RpcClientException`].
    kind: RpcClientExceptionKind,

    /// Error message describing the problem.
    message: Cow<'static, str>,

    /// [`platform::Error`] that caused this [`RpcClientException`].
    cause: Option<platform::Error>,

    /// Stacktrace of this [`RpcClientException`].
    trace: Trace,
}

impl RpcClientException {
    /// Creates a new [`RpcClientException`] from the provided error `kind`,
    /// `message`, optional `cause` and `trace`.
    #[inline]
    #[must_use]
    pub fn new<M: Into<Cow<'static, str>>>(
        kind: RpcClientExceptionKind,
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

impl From<RpcClientException> for DartError {
    #[inline]
    fn from(err: RpcClientException) -> Self {
        unsafe {
            Self::new(NEW_RPC_CLIENT_EXCEPTION_CALLER.unwrap()(
                err.kind,
                string_into_c_str(err.message.into_owned()),
                err.cause.map(DartError::from).into(),
                string_into_c_str(err.trace.to_string()),
            ))
        }
    }
}

/// Exception thrown when the requested media state transition could not be
/// performed.
pub struct MediaStateTransitionException {
    /// Error message describing the problem.
    message: Cow<'static, str>,

    /// Stacktrace of this [`MediaStateTransitionException`].
    trace: Trace,
}

impl MediaStateTransitionException {
    /// Creates a new [`MediaStateTransitionException`] from the provided error
    /// `message` and `trace`.
    #[inline]
    #[must_use]
    pub fn new<T: Into<Cow<'static, str>>>(message: T, trace: Trace) -> Self {
        Self {
            message: message.into(),
            trace,
        }
    }
}

impl From<MediaStateTransitionException> for DartError {
    #[inline]
    fn from(err: MediaStateTransitionException) -> Self {
        unsafe {
            Self::new(NEW_MEDIA_STATE_TRANSITION_EXCEPTION_CALLER.unwrap()(
                string_into_c_str(err.message.into_owned()),
                string_into_c_str(err.trace.to_string()),
            ))
        }
    }
}

/// Jason's internal exception.
///
/// This is either a programmatic error or some unexpected platform component
/// failure that cannot be handled in any way.
///
/// It can be converted into a [`DartError`] and passed to Dart.
pub struct InternalException {
    /// Error message describing the problem.
    message: Cow<'static, str>,

    /// [`platform::Error`] that caused this [`RpcClientException`].
    cause: Option<platform::Error>,

    /// Stacktrace of this [`InternalException`].
    trace: Trace,
}

impl InternalException {
    /// Creates a new [`InternalException`] from the provided error `message`,
    /// `trace` and an optional `cause`.
    #[inline]
    #[must_use]
    pub fn new<T: Into<Cow<'static, str>>>(
        message: T,
        cause: Option<platform::Error>,
        trace: Trace,
    ) -> Self {
        Self {
            message: message.into(),
            trace,
            cause,
        }
    }
}

impl From<InternalException> for DartError {
    #[inline]
    fn from(err: InternalException) -> Self {
        unsafe {
            Self::new(NEW_INTERNAL_EXCEPTION_CALLER.unwrap()(
                string_into_c_str(err.message.into_owned()),
                err.cause.map(DartError::from).into(),
                string_into_c_str(err.trace.to_string()),
            ))
        }
    }
}

/// Errors occurring in [`RoomHandle::set_local_media_settings()`][1] method.
///
/// It can be converted into a [`DartError`] and passed to Dart.
///
/// [1]: crate::api::RoomHandle::set_local_media_settings
pub struct MediaSettingsUpdateException {
    /// Error message describing the problem.
    message: Cow<'static, str>,

    /// Original [`ChangeMediaStateError`] that was encountered while updating
    /// local media settings.
    cause: Traced<ChangeMediaStateError>,

    /// Whether media settings were successfully rolled back after new settings
    /// application failed.
    rolled_back: bool,
}

impl MediaSettingsUpdateException {
    /// Creates a new [`MediaSettingsUpdateException`] from the provided error
    /// `message`, `cause` and `rolled_back` property.
    #[inline]
    #[must_use]
    pub fn new<T: Into<Cow<'static, str>>>(
        message: T,
        cause: Traced<ChangeMediaStateError>,
        rolled_back: bool,
    ) -> Self {
        Self {
            message: message.into(),
            rolled_back,
            cause,
        }
    }
}

impl From<MediaSettingsUpdateException> for DartError {
    #[inline]
    fn from(err: MediaSettingsUpdateException) -> Self {
        unsafe {
            Self::new(NEW_MEDIA_SETTINGS_UPDATE_EXCEPTION_CALLER.unwrap()(
                string_into_c_str(err.message.into_owned()),
                err.cause.into(),
                err.rolled_back as u8,
            ))
        }
    }
}
