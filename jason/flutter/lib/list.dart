import 'ffi.dart' as ffi;
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions() {
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_Array__get')(
      Pointer.fromFunction<Handle Function(Handle, int)>(get)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_Array__length')(
      Pointer.fromFunction<int Function(Handle)>(length)
  );
}

Object get(Object arr, int i) {
  if (arr is List) {
    return arr[i];
  } else {
    throw Exception("Unexpected Object provided from Rust: " + arr.runtimeType.toString());
  }
}

int length(Object arr) {
  if (arr is List) {
    return arr.length;
  } else {
    throw Exception("Unexpected Object provided from Rust: " + arr.runtimeType.toString());
  }
}