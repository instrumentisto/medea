import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'native_string.dart';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_new_handler_detached_error_caller')(
      Pointer.fromFunction<Handle Function(Pointer<Utf8>)>(
          newHandlerDetachedError));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_new_media_manager_exception_caller')(
      Pointer.fromFunction<
          Handle Function(
              Pointer<Utf8>, Handle, Pointer<Utf8>)>(newMediaManagerException));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_new_argument_error_caller')(
      Pointer.fromFunction<Handle Function(Pointer<Utf8>)>(newArgumentError));
}

class HandlerDetachedError extends Error {
  late String nativeStackTrace;

  HandlerDetachedError(this.nativeStackTrace);
}

Object newHandlerDetachedError(Pointer<Utf8> stackTrace) {
  return HandlerDetachedError(stackTrace.nativeStringToDartString());
}

class MediaManagerException implements Exception {
  late String message;
  late dynamic cause;
  late String nativeStackTrace;

  MediaManagerException(this.message, this.cause, this.nativeStackTrace);
}

Object newMediaManagerException(
    Pointer<Utf8> message, Object? cause, Pointer<Utf8> nativeStackTrace) {
  return MediaManagerException(message.nativeStringToDartString(), cause,
      nativeStackTrace.nativeStringToDartString());
}

Object newArgumentError(Pointer<Utf8> message) {
  return ArgumentError(message.nativeStringToDartString());
}
