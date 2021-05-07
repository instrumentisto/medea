import 'package:ffi/ffi.dart';
import 'dart:ffi';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_new_exception_function')(
      Pointer.fromFunction<Handle Function(Pointer<Utf8>)>(newException));
}

Object newException(Pointer<Utf8> message) {
  return Error();
}
