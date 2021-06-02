import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'foreign_value.dart';
import 'native_string.dart';

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
      'register_new_media_manager_exception_caller')(Pointer.fromFunction<
          Handle Function(Uint8, Pointer<Utf8>, ForeignValue, Pointer<Utf8>)>(
      _newMediaManagerException));
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

/// Creates a new [MediaManagerException] with the provided error [kind],
/// [message], [cause] and [stacktrace].
Object _newMediaManagerException(int kind, Pointer<Utf8> message,
    ForeignValue cause, Pointer<Utf8> stacktrace) {
  return MediaManagerException(
      MediaManagerExceptionKind.values[kind],
      message.nativeStringToDartString(),
      cause.toDart(),
      stacktrace.nativeStringToDartString());
}

/// Exception thrown when accessing media devices.
class MediaManagerException implements Exception {
  /// Concrete error kind of this [MediaManagerException].
  late MediaManagerExceptionKind kind;

  /// Error message describing the problem.
  late String message;

  /// Dart [Exception] or [Error] that caused this [MediaManagerException].
  late Object? cause;

  /// Native stacktrace.
  late String nativeStackTrace;

  /// Instantiates a new [MediaManagerException].
  MediaManagerException(
      this.kind, this.message, this.cause, this.nativeStackTrace);
}

/// Possible error kinds of a [MediaManagerException].
enum MediaManagerExceptionKind {
  /// Occurs if the [getUserMedia()][1] request failed.
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
