import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'foreign_value.dart';
import 'native_string.dart';
import 'unbox_handle.dart';

/// Registers functions allowing Rust to create Dart [Exception]s and [Error]s.
void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_new_argument_error_caller')(
      Pointer.fromFunction<
          Handle Function(
              ForeignValue, Pointer<Utf8>, Pointer<Utf8>)>(_newArgumentError));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_new_state_error_caller')(
      Pointer.fromFunction<Handle Function(Pointer<Utf8>)>(_newStateError));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_new_format_exception_caller')(
      Pointer.fromFunction<Handle Function(Pointer<Utf8>)>(
          _newFormatException));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      'register_new_local_media_init_exception_caller')(Pointer.fromFunction<
          Handle Function(Uint8, Pointer<Utf8>, ForeignValue, Pointer<Utf8>)>(
      _newLocalMediaInitException));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_new_enumerate_devices_exception_caller')(
      Pointer.fromFunction<Handle Function(Pointer<Handle>, Pointer<Utf8>)>(
          _newEnumerateDevicesException));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      'register_new_rpc_client_exception_caller')(Pointer.fromFunction<
          Handle Function(Uint8, Pointer<Utf8>, ForeignValue, Pointer<Utf8>)>(
      _newRpcClientException));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_new_media_state_transition_exception_caller')(
      Pointer.fromFunction<Handle Function(Pointer<Utf8>, Pointer<Utf8>)>(
          _newMediaStateTransitionException));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      'register_new_internal_exception_caller')(Pointer.fromFunction<
          Handle Function(Pointer<Utf8>, ForeignValue, Pointer<Utf8>)>(
      _newInternalException));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_new_media_settings_update_exception_caller')(
      Pointer.fromFunction<
          Handle Function(Pointer<Utf8>, Pointer<Handle>,
              Uint8)>(_newMediaSettingsUpdateException));
}

/// Creates a new [ArgumentError] from the provided invalid [value], its [name]
/// and the [message] describing the problem.
Object _newArgumentError(
    ForeignValue value, Pointer<Utf8> name, Pointer<Utf8> message) {
  return ArgumentError.value(value.toDart(), name.nativeStringToDartString(),
      message.nativeStringToDartString());
}

/// Creates a new [StateError] with the provided [message].
Object _newStateError(Pointer<Utf8> message) {
  return StateError(message.nativeStringToDartString());
}

/// Creates a new [FormatException] with the provided [message].
Object _newFormatException(Pointer<Utf8> message) {
  return FormatException(message.nativeStringToDartString());
}

/// Creates a new [LocalMediaInitException] with the provided error [kind],
/// [message], [cause] and [stacktrace].
Object _newLocalMediaInitException(int kind, Pointer<Utf8> message,
    ForeignValue cause, Pointer<Utf8> stacktrace) {
  return LocalMediaInitException(
      LocalMediaInitExceptionKind.values[kind],
      message.nativeStringToDartString(),
      cause.toDart(),
      stacktrace.nativeStringToDartString());
}

/// Creates a new [EnumerateDevicesException] with the provided error [cause]
/// and [stacktrace].
Object _newEnumerateDevicesException(
    Pointer<Handle> cause, Pointer<Utf8> stacktrace) {
  return EnumerateDevicesException(
      unboxDartHandle(cause), stacktrace.nativeStringToDartString());
}

/// Creates a new [RpcClientException] with the provided error [kind],
/// [message], [cause] and [stacktrace].
Object _newRpcClientException(int kind, Pointer<Utf8> message,
    ForeignValue cause, Pointer<Utf8> stacktrace) {
  return RpcClientException(
      RpcClientExceptionKind.values[kind],
      message.nativeStringToDartString(),
      cause.toDart(),
      stacktrace.nativeStringToDartString());
}

/// Creates a new [MediaStateTransitionException] with the provided error
/// [message] and [stacktrace].
Object _newMediaStateTransitionException(
    Pointer<Utf8> message, Pointer<Utf8> stacktrace) {
  return MediaStateTransitionException(message.nativeStringToDartString(),
      stacktrace.nativeStringToDartString());
}

/// Creates a new [InternalException] with the provided error [message], error
/// [cause] and [stacktrace].
Object _newInternalException(
    Pointer<Utf8> message, ForeignValue cause, Pointer<Utf8> stacktrace) {
  return InternalException(message.nativeStringToDartString(), cause.toDart(),
      stacktrace.nativeStringToDartString());
}

/// Creates a new [MediaSettingsUpdateException] with the provided error
/// [message], error [cause] and [rolledBack] property.
Object _newMediaSettingsUpdateException(
    Pointer<Utf8> message, Pointer<Handle> cause, int rolledBack) {
  return MediaSettingsUpdateException(message.nativeStringToDartString(),
      unboxDartHandle(cause), rolledBack > 0);
}

/// Exception thrown when local media acquisition fails.
class LocalMediaInitException implements Exception {
  /// Concrete error kind of this [LocalMediaInitException].
  late LocalMediaInitExceptionKind kind;

  /// Error message describing the problem.
  late String message;

  /// Dart [Exception] or [Error] that caused this [LocalMediaInitException].
  late Object? cause;

  /// Native stacktrace.
  late String nativeStackTrace;

  /// Instantiates a new [LocalMediaInitException].
  LocalMediaInitException(
      this.kind, this.message, this.cause, this.nativeStackTrace);
}

/// Possible error kinds of a [LocalMediaInitException].
enum LocalMediaInitExceptionKind {
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

/// Exception thrown when cannot get info about connected [MediaDevices][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams#mediadevices
class EnumerateDevicesException implements Exception {
  /// Dart [Exception] or [Error] that caused this [EnumerateDevicesException].
  late Object cause;

  /// Native stacktrace.
  late String nativeStackTrace;

  /// Instantiates a new [EnumerateDevicesException].
  EnumerateDevicesException(this.cause, this.nativeStackTrace);
}

/// Exceptions thrown from `Jason`'s `RpcClient` which implements messaging with
/// media server.
class RpcClientException implements Exception {
  /// Concrete error kind of this [RpcClientException].
  late RpcClientExceptionKind kind;

  /// Error message describing the problem.
  late String message;

  /// Dart [Exception] or [Error] that caused this [RpcClientException].
  late Object? cause;

  /// Native stacktrace.
  late String nativeStackTrace;

  /// Instantiates a new [RpcClientException].
  RpcClientException(
      this.kind, this.message, this.cause, this.nativeStackTrace);
}

/// Possible error kinds of a [RpcClientException].
enum RpcClientExceptionKind {
  /// Connection with a server was lost.
  ///
  /// This usually means that some transport error occurred, so a client can
  /// continue performing reconnecting attempts.
  ConnectionLost,

  /// Could not authorize an RPC session.
  ///
  /// This usually means that authentication data a client provides is obsolete.
  AuthorizationFailed,

  /// RPC session has been finished. This is a terminal state.
  SessionFinished,
}

/// Exception thrown when the requested media state transition could not be
/// performed.
class MediaStateTransitionException implements Exception {
  /// Error message describing the problem.
  late String message;

  /// Native stacktrace.
  late String nativeStackTrace;

  /// Instantiates a new [MediaStateTransitionException].
  MediaStateTransitionException(this.message, this.nativeStackTrace);
}

/// Jason's internal exception.
///
/// This is either a programmatic error or some unexpected platform component
/// failure that cannot be handled in any way.
class InternalException implements Exception {
  /// Error message describing the problem.
  late String message;

  /// Dart [Exception] or [Error] that caused this [InternalException].
  late Object? cause;

  /// Native stacktrace.
  late String nativeStackTrace;

  /// Instantiates a new [InternalException].
  InternalException(this.message, this.cause, this.nativeStackTrace);
}

/// Exception that might happen when updating local media settings via
/// `RoomHandle.setLocalMediaSettings`.
class MediaSettingsUpdateException implements Exception {
  /// Error message describing the problem.
  late String message;

  /// The reason why media settings update failed.
  ///
  /// Possible exception kinds are:
  /// - [StateError] if an underlying `RoomHandle` object has been disposed.
  /// - [LocalMediaInitException] if a request of platform media devices access
  ///   failed.
  /// - [MediaStateTransitionException] if transition is prohibited by tracks
  ///   configuration or explicitly denied by server.
  /// - [InternalException] in case of a programmatic error or some unexpected
  ///   platform component failure.
  late Object updateException;

  /// Whether media settings were successfully rolled back after new settings
  /// application failed.
  late bool rolledBack;

  /// Instantiates a new [MediaSettingsUpdateException].
  MediaSettingsUpdateException(
      this.message, this.updateException, this.rolledBack);
}
