
import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'native_string.dart';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      'register_new_handler_detached_error')(
      Pointer.fromFunction<Handle Function(Pointer<Utf8>)>(newHandlerDetachedError));
}

class HandlerDetachedError extends Error {

  late String nativeStackTrace;

  HandlerDetachedError(this.nativeStackTrace);
}

class MediaManagerException implements Exception {


  late String nativeStackTrace;

  MediaManagerException(this.nativeStackTrace);
}

Object newHandlerDetachedError(Pointer<Utf8> stackTrace) {
  return HandlerDetachedError(stackTrace.nativeStringToDartString());
}

