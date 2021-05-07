import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_Array__get')(
      Pointer.fromFunction<Handle Function(Handle, Int32)>(get));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_Array__length')(
      Pointer.fromFunction<Int32 Function(Handle)>(len, 0));
}

Object get(Object arr, int i) {
  if (arr is List) {
    return arr[i];
  } else {
    throw Exception(
        "Unexpected Object provided from Rust: " + arr.runtimeType.toString());
  }
}

int len(Object arr) {
  if (arr is List) {
    return arr.length;
  } else {
    throw Exception(
        "Unexpected Object provided from Rust: " + arr.runtimeType.toString());
  }
}
