import 'dart:ffi';

import 'foreign_value.dart';
import 'unbox_handle.dart';

class Result extends Struct {
  /// Index of the [DartValueFields] union field. `0` goes for `Void`.
  @Uint8()
  external int _tag;

  /// Actual [ForeignValue] payload.
  external ResultFields _payload;

  /// Returns Dart representation of the underlying foreign value.
  ///
  /// Returns `null` if underlying value is `void` or `()`.
  dynamic unwrap() {
    if (_tag == 0) {
      return _payload.ok.toDart();
    } else {
      throw unboxDartHandle(_payload.errPtr);
    }
  }
}

class ResultFields extends Union {
  /// Success [ForeignValue].
  external ForeignValue ok;

  /// [Error] value.
  external Pointer<Handle> errPtr;
}
