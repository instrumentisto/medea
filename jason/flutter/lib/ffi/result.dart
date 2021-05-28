import 'dart:ffi';

import 'foreign_value.dart';
import 'unbox_handle.dart';

/// Class representing either success or failure.
///
/// Implements error propagation from Rust to Dart.
class Result extends Struct {
  /// Index of the used [_ResultFields] union field.
  @Uint8()
  external int _tag;

  /// Actual [Result] payload.
  external _ResultFields _payload;

  /// Returns the underlying Dart value, which is an [Object] in case of
  /// success, or throws an [Exception] or an [Error] in case of failure.
  dynamic unwrap() {
    if (_tag == 0) {
      return _payload.ok.toDart();
    } else {
      throw unboxDartHandle(_payload.errPtr);
    }
  }
}

/// Possible fields of a [Result].
class _ResultFields extends Union {
  /// Success [ForeignValue].
  external ForeignValue ok;

  /// Failure value.
  external Pointer<Handle> errPtr;
}
