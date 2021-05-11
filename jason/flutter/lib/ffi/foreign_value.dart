import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'native_string.dart';

class ForeignValue extends Struct {
  @Uint8()
  external int _tag;
  external DartValueFields _payload;

  dynamic toDart() {
    switch (_tag) {
      case 0:
        return;
      case 1:
        return _payload.ptr;
      case 2:
        return _payload.string.nativeStringToDartString();
      case 3:
        return _payload.number;
      default:
        throw TypeError();
    }
  }
}

class DartValueFields extends Union {
  external Pointer ptr;
  external Pointer<Utf8> string;
  @Int64()
  external int number;
}
