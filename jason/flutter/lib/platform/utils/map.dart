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
  try {
    map = map as Map;
    map[key.toDartString()] = value;
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

void remove(Object map, Pointer<Utf8> key) {
  try {
    if (map is Map) {
      map.remove(key.toDartString());
    } else {
      throw Exception(
          "Unexpected Object provided from Rust: " + map.runtimeType.toString());
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

Object constructInt(int value) {
  try {
    return value;
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

Object constructString(Pointer<Utf8> str) {
  try {
    return str.toDartString();
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}
