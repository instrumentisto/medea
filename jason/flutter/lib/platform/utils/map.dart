import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_DartMap__new')(
      Pointer.fromFunction<Handle Function()>(construct));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_DartMap__set')(
      Pointer.fromFunction<Void Function(Handle, Pointer<Utf8>, Handle)>(set));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_Int__new')(
      Pointer.fromFunction<Handle Function(Int32)>(constructInt));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_String__new')(
      Pointer.fromFunction<Handle Function(Pointer<Utf8>)>(constructString));
}

Object construct() {
  return Map();
}

void set(Object map, Pointer<Utf8> key, Object value) {
  map = map as Map;
  map[key.toDartString()] = value;
}

void remove(Object map, Pointer<Utf8> key) {
  if (map is Map) {
    map.remove(key.toDartString());
  } else {
    throw Exception(
        "Unexpected Object provided from Rust: " + map.runtimeType.toString());
  }
}

Object constructInt(int value) {
  return value;
}

Object constructString(Pointer<Utf8> str) {
  return str.toDartString();
}
