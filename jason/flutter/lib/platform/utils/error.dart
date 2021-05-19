import 'package:ffi/ffi.dart';
import 'dart:ffi';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_DartError__name')(
      Pointer.fromFunction<Pointer<Utf8> Function(Handle)>(name));
  ;
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_DartError__message')(
      Pointer.fromFunction<Pointer<Utf8> Function(Handle)>(message));
}

Pointer<Utf8> name(Object exception) {
  exception = exception as Exception;
  return exception.runtimeType.toString().toNativeUtf8();
}

Pointer<Utf8> message(Object exception) {
  exception = exception as Exception;
  return exception.toString().toNativeUtf8();
}
