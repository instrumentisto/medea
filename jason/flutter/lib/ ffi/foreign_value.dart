import 'dart:ffi';

import 'package:ffi/ffi.dart';

class DartValue extends Struct {
  @Uint8()
  int tag;
  DartValueFields payload;

  dynamic parse() {
    switch (tag) {
      case 0:
        return;
      case 1:
        return payload.ptr;
      case 2:
        return payload.string.toDartString();
      case 3:
        return payload.number;
    }
  }
}

class DartValueFields extends Union {
  external Pointer ptr;
  external Pointer<Utf8> string;
  @Int64()
  external int number;
}
