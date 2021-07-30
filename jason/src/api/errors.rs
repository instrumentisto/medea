//! Implementations of the API errors which can be throwed.

use std::borrow::Cow;

#[cfg(not(target_os = "android"))]
use wasm_bindgen::prelude::wasm_bindgen;

use tracerr::{Trace, Traced};

use crate::{
    api::Error,
    connection,
    media::{
        EnumerateDevicesError, GetDisplayMediaError, GetUserMediaError,
        InitLocalTracksError,
    },
    peer::{
        sender::CreateError, InsertLocalTracksError, LocalMediaError,
        UpdateLocalStreamError,
    },
    platform,
    room::{
        self, ChangeMediaStateError, ConstraintsUpdateError, RoomJoinError,
    },
    rpc::{rpc_session::ConnectionLostReason, ReconnectError, SessionError},
    utils::Caused,
};

/// Error thrown when the operation wasn't allowed by the current state of the
/// object.
#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
pub struct StateError {
    /// Message describing the problem.
    message: Cow<'static, str>,

    /// Stacktrace of this [`StateError`].
    trace: Trace,
}

impl StateError {
    /// Creates a new [`StateError`] with the provided `message` and `trace`.
    #[inline]
    #[must_use]
    pub fn new<T: Into<Cow<'static, str>>>(message: T, trace: Trace) -> Self {
        Self {
            message: message.into(),
            trace,
        }
    }
}

#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
impl StateError {
    /// Returns message describing the problem.
    #[must_use]
    pub fn message(&self) -> String {
        self.message.to_string()
    }

    /// Returns native stacktrace of this [`StateError`].
    #[must_use]
    pub fn trace(&self) -> String {
        self.trace.to_string()
    }
}

/// Possible error kinds of a [`LocalMediaInitException`].
#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
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
#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
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

#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
impl LocalMediaInitException {
    /// Returns concrete error kind of this [`LocalMediaInitException`].
    #[must_use]
    pub fn kind(&self) -> LocalMediaInitExceptionKind {
        self.kind
    }

    /// Returns error message describing the problem.
    #[must_use]
    pub fn message(&self) -> String {
        self.message.to_string()
    }

    /// Returns [`platform::Error`] that caused this
    /// [`LocalMediaInitException`].
    #[must_use]
    pub fn cause(&self) -> Option<platform::Error> {
        self.cause.clone()
    }

    /// Returns stacktrace of this [`LocalMediaInitException`].
    #[must_use]
    pub fn trace(&self) -> String {
        self.trace.to_string()
    }
}

/// Exception thrown when cannot get info of available media devices.
#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
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

#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
impl EnumerateDevicesException {
    /// Returns [`platform::Error`] that caused this
    /// [`EnumerateDevicesException`].
    #[must_use]
    pub fn cause(&self) -> platform::Error {
        self.cause.clone()
    }

    /// Returns stacktrace of this [`EnumerateDevicesException`].
    #[must_use]
    pub fn trace(&self) -> String {
        self.trace.to_string()
    }
}

/// Possible error kinds of a [`RpcClientException`].
#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
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
#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
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

#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
impl RpcClientException {
    /// Returns concrete error kind of this [`RpcClientException`].
    #[must_use]
    pub fn kind(&self) -> RpcClientExceptionKind {
        self.kind
    }

    /// Returns error message describing the problem.
    #[must_use]
    pub fn message(&self) -> String {
        self.message.to_string()
    }

    /// Returns [`platform::Error`] that caused this [`RpcClientException`].
    #[must_use]
    pub fn cause(&self) -> Option<platform::Error> {
        self.cause.clone()
    }

    /// Returns stacktrace of this [`RpcClientException`].
    #[must_use]
    pub fn trace(&self) -> String {
        self.trace.to_string()
    }
}

/// Jason's internal exception.
///
/// This is either a programmatic error or some unexpected platform component
/// failure that cannot be handled in any way.
#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
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

#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
impl InternalException {
    /// Returns error message describing the problem.
    #[must_use]
    pub fn message(&self) -> String {
        self.message.to_string()
    }

    /// Returns [`platform::Error`] that caused this [`RpcClientException`].
    #[must_use]
    pub fn cause(&self) -> Option<platform::Error> {
        self.cause.clone()
    }

    /// Returns stacktrace of this [`InternalException`].
    #[must_use]
    pub fn trace(&self) -> String {
        self.trace.to_string()
    }
}

/// Exception thrown when a string or some other data doesn't have an expected
/// format and cannot be parsed or processed.
#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
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

#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
impl FormatException {
    /// Returns describing of the problem.
    #[must_use]
    pub fn message(&self) -> String {
        self.0.to_string()
    }
}

/// Exception thrown when the requested media state transition could not be
/// performed.
#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
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

#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
impl MediaStateTransitionException {
    /// Returns error message describing the problem.
    #[must_use]
    pub fn message(&self) -> String {
        self.message.to_string()
    }

    /// Returns stacktrace of this [`MediaStateTransitionException`].
    #[must_use]
    pub fn trace(&self) -> String {
        self.trace.to_string()
    }
}

/// Errors occurring in [`RoomHandle::set_local_media_settings()`][1] method.
///
/// It can be converted into a [`DartError`] and passed to Dart.
///
/// [1]: crate::api::RoomHandle::set_local_media_settings
#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
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

#[cfg_attr(not(target_os = "android"), wasm_bindgen)]
impl MediaSettingsUpdateException {
    /// Returns error message describing the problem.
    #[must_use]
    pub fn message(&self) -> String {
        self.message.to_string()
    }

    /// Returns original [`ChangeMediaStateError`] that was encountered while
    /// updating local media settings.
    #[must_use]
    pub fn cause(&self) -> Error {
        self.cause.clone().into()
    }

    /// Returns whether media settings were successfully rolled back after new
    /// settings application failed.
    #[must_use]
    pub fn rolled_back(&self) -> bool {
        self.rolled_back
    }
}

impl From<Traced<connection::HandleDetachedError>> for Error {
    fn from(err: Traced<connection::HandleDetachedError>) -> Self {
        let (err, trace) = err.into_parts();

        StateError::new(err.to_string(), trace).into()
    }
}

impl From<Traced<room::HandleDetachedError>> for Error {
    fn from(err: Traced<room::HandleDetachedError>) -> Self {
        let (err, trace) = err.into_parts();

        StateError::new(err.to_string(), trace).into()
    }
}

impl From<Traced<EnumerateDevicesError>> for Error {
    #[inline]
    fn from(err: Traced<EnumerateDevicesError>) -> Self {
        let (err, stacktrace) = err.into_parts();
        EnumerateDevicesException::new(err.into(), stacktrace).into()
    }
}

impl From<Traced<InitLocalTracksError>> for Error {
    fn from(err: Traced<InitLocalTracksError>) -> Self {
        use GetDisplayMediaError as Gdm;
        use GetUserMediaError as Gum;
        use InitLocalTracksError as Err;
        use LocalMediaInitExceptionKind as Kind;

        let (err, stacktrace) = err.into_parts();
        let message = err.to_string();

        let (kind, cause) = match err {
            Err::Detached => {
                return StateError::new(message, stacktrace).into()
            }
            Err::GetUserMediaFailed(Gum::PlatformRequestFailed(cause)) => {
                (Kind::GetUserMediaFailed, Some(cause))
            }
            Err::GetDisplayMediaFailed(Gdm::PlatformRequestFailed(cause)) => {
                (Kind::GetDisplayMediaFailed, Some(cause))
            }
            Err::GetUserMediaFailed(Gum::LocalTrackIsEnded(_))
            | Err::GetDisplayMediaFailed(Gdm::LocalTrackIsEnded(_)) => {
                (Kind::LocalTrackIsEnded, None)
            }
        };

        LocalMediaInitException::new(kind, message, cause, stacktrace).into()
    }
}

impl From<Traced<ReconnectError>> for Error {
    #[inline]
    fn from(err: Traced<ReconnectError>) -> Self {
        let (err, trace) = err.into_parts();

        match err {
            ReconnectError::Detached => {
                StateError::new(err.to_string(), trace).into()
            }
            ReconnectError::Session(err) => {
                Traced::from_parts(err, trace).into()
            }
        }
    }
}

impl From<Traced<SessionError>> for Error {
    #[allow(clippy::option_if_let_else)]
    fn from(err: Traced<SessionError>) -> Self {
        use ConnectionLostReason as Reason;
        use RpcClientExceptionKind as Kind;
        use SessionError as SE;

        let (err, trace) = err.into_parts();
        let message = err.to_string();

        let mut cause = None;
        let kind = match err {
            SE::SessionFinished(_) => Some(Kind::SessionFinished),
            SE::NoCredentials
            | SE::SessionUnexpectedlyDropped
            | SE::NewConnectionInfo => None,
            SE::RpcClient(err) => {
                cause = err.cause();
                None
            }
            SE::AuthorizationFailed => Some(Kind::AuthorizationFailed),
            SE::ConnectionLost(reason) => {
                if let Reason::ConnectError(err) = reason {
                    cause = err.into_inner().cause()
                };
                Some(Kind::ConnectionLost)
            }
        };

        if let Some(rpc_kind) = kind {
            RpcClientException::new(rpc_kind, message, cause, trace).into()
        } else {
            InternalException::new(message, cause, trace).into()
        }
    }
}

impl From<Traced<RoomJoinError>> for Error {
    #[inline]
    fn from(err: Traced<RoomJoinError>) -> Self {
        let (err, trace) = err.into_parts();
        let message = err.to_string();

        match err {
            RoomJoinError::Detached | RoomJoinError::CallbackNotSet(_) => {
                StateError::new(message, trace).into()
            }
            RoomJoinError::ConnectionInfoParse(_) => {
                FormatException::new(message).into()
            }
            RoomJoinError::SessionError(err) => {
                Traced::from_parts(err, trace).into()
            }
        }
    }
}

impl From<Traced<ChangeMediaStateError>> for Error {
    #[inline]
    fn from(err: Traced<ChangeMediaStateError>) -> Self {
        let (err, trace) = err.into_parts();
        let message = err.to_string();

        match err {
            ChangeMediaStateError::Detached => {
                StateError::new(err.to_string(), trace).into()
            }
            ChangeMediaStateError::CouldNotGetLocalMedia(err) => {
                Traced::from_parts(err, trace).into()
            }
            ChangeMediaStateError::ProhibitedState(_)
            | ChangeMediaStateError::TransitionIntoOppositeState(_)
            | ChangeMediaStateError::InvalidLocalTracks(_) => {
                MediaStateTransitionException::new(message, trace).into()
            }
            ChangeMediaStateError::InsertLocalTracksError(_) => {
                InternalException::new(message, None, trace).into()
            }
        }
    }
}

impl From<ConstraintsUpdateError> for Error {
    #[inline]
    fn from(err: ConstraintsUpdateError) -> Self {
        let message = err.to_string();

        let (err, rolled_back) = match err {
            ConstraintsUpdateError::Recovered(err) => (err, true),
            ConstraintsUpdateError::RecoverFailed {
                recover_reason, ..
            } => (recover_reason, false),
            ConstraintsUpdateError::Errored(err) => (err, false),
        };

        MediaSettingsUpdateException::new(message, err, rolled_back).into()
    }
}

impl From<Traced<LocalMediaError>> for Error {
    fn from(err: Traced<LocalMediaError>) -> Self {
        use InsertLocalTracksError as IE;
        use LocalMediaError as ME;
        use UpdateLocalStreamError as UE;

        let (err, trace) = err.into_parts();
        let message = err.to_string();

        match err {
            ME::UpdateLocalStreamError(err) => match err {
                UE::CouldNotGetLocalMedia(err) => {
                    Traced::from_parts(err, trace).into()
                }
                UE::InvalidLocalTracks(_)
                | UE::InsertLocalTracksError(
                    IE::InvalidMediaTrack | IE::NotEnoughTracks,
                ) => MediaStateTransitionException::new(message, trace).into(),
                UE::InsertLocalTracksError(IE::CouldNotInsertLocalTrack(_)) => {
                    InternalException::new(message, None, trace).into()
                }
            },
            ME::SenderCreateError(CreateError::TransceiverNotFound(_)) => {
                InternalException::new(message, None, trace).into()
            }
            ME::SenderCreateError(CreateError::CannotDisableRequiredSender) => {
                MediaStateTransitionException::new(message, trace).into()
            }
        }
    }
}
