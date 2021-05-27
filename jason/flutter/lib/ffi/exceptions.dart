import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'native_string.dart';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_new_argument_error_caller')(
      Pointer.fromFunction<Handle Function(Pointer<Utf8>)>(_newArgumentError));
}

Object _newArgumentError(Pointer<Utf8> message) {
  return ArgumentError(message.nativeStringToDartString());
}
