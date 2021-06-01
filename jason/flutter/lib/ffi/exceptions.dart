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
          'register_new_media_manager_exception_caller')(
      Pointer.fromFunction<
          Handle Function(Pointer<Utf8>, Pointer<Utf8>, ForeignValue,
              Pointer<Utf8>)>(_newMediaManagerException));
}

/// Create a new [ArgumentError] from the provided invalid [value], its [name]
/// and the [message] describing the problem.
Object _newArgumentError(
    ForeignValue value, Pointer<Utf8> name, Pointer<Utf8> message) {
  return ArgumentError.value(value.toDart(), name.nativeStringToDartString(),
      message.nativeStringToDartString());
}

/// Create a new [StateError] with the provided error [message].
Object _newStateError(Pointer<Utf8> message) {
  return StateError(message.nativeStringToDartString());
}

class MediaManagerException implements Exception {
  late String name;
  late String message;
  late dynamic cause;
  late String nativeStackTrace;

  MediaManagerException(
      this.name, this.message, this.cause, this.nativeStackTrace);
}

Object _newMediaManagerException(Pointer<Utf8> name, Pointer<Utf8> message,
    ForeignValue cause, Pointer<Utf8> nativeStackTrace) {
  return MediaManagerException(
      name.nativeStringToDartString(),
      message.nativeStringToDartString(),
      cause.toDart(),
      nativeStackTrace.nativeStringToDartString());
}
