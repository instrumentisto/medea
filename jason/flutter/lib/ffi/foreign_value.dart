import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'native_string.dart';

class ForeignValue extends Struct {
  /// Index of the [DartValueFields] union field. `0` goes for `Void`.
  @Uint8()
  external int _tag;

  /// Actual [ForeignValue] payload.
  external DartValueFields _payload;

  /// Returns Dart representation of the underlying foreign value.
  ///
  /// Returns `null` if underlying value is `void` or `()`.
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
  /// [Pointer] to some Rust object.
  external Pointer ptr;

  /// [Pointer] to native string.
  external Pointer<Utf8> string;

  /// Numeric value.
  @Int64()
  external int number;
}
