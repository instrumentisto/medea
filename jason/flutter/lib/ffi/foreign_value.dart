import 'dart:ffi';

import 'package:ffi/ffi.dart';

import '../util/nullable_pointer.dart';
import '../util/move_semantic.dart';
import '../jason.dart';
import 'native_string.dart';

typedef _unboxDartHandle_C = Handle Function(Pointer<Handle>);
typedef _unboxDartHandle_Dart = Object Function(Pointer<Handle>);

final _unboxDartHandle =
    dl.lookupFunction<_unboxDartHandle_C, _unboxDartHandle_Dart>(
        'unbox_dart_handle');

/// Type-erased value that can be transferred via FFI boundaries.
class ForeignValue extends Struct {
  /// Index of the [DartValueFields] union field.
  ///
  /// `0` goes for no value.
  @Uint8()
  external int _tag;

  /// Actual [ForeignValue] payload.
  external DartValueFields _payload;

  /// Private constructor.
  ///
  /// This class is a reference backed by native memory, so it cant be
  /// instantiated like a normal Dart class.
  ForeignValue._();

  /// Returns Dart representation of the underlying foreign value.
  ///
  /// Returns `null` if underlying value is `None`.
  dynamic toDart() {
    switch (_tag) {
      case 0:
        return;
      case 1:
        return _payload.ptr;
      case 2:
        return _unboxDartHandle(_payload.handlePtr);
      case 3:
        return _payload.string.nativeStringToDartString();
      case 4:
        return _payload.number;
      default:
        throw TypeError();
    }
  }

  /// Allocates [ForeignValue] with no value.
  ///
  /// This can be used when calling native function with optional argument.
  static Pointer<ForeignValue> none() {
    return calloc<ForeignValue>();
  }

  /// Allocates [ForeignValue] with the provided pointer to some Rust object.
  static Pointer<ForeignValue> fromPtr(NullablePointer ptr) {
    var fVal = calloc<ForeignValue>();
    fVal.ref._tag = 1;
    fVal.ref._payload.ptr = ptr.getInnerPtr();
    return fVal;
  }

  /// Allocates [ForeignValue] with the provided [String].
  static Pointer<ForeignValue> fromString(String str) {
    var fVal = calloc<ForeignValue>();
    fVal.ref._tag = 3;
    fVal.ref._payload.ptr = str.toNativeUtf8();
    return fVal;
  }

  /// Allocates [ForeignValue] with the provided [int] value.
  static Pointer<ForeignValue> fromInt(int num) {
    var fVal = calloc<ForeignValue>();
    fVal.ref._tag = 4;
    fVal.ref._payload.number = num;
    return fVal;
  }
}

extension ForeignValuePointer on Pointer<ForeignValue> {
  /// Releases memory allocated on the native heap.
  @moveSemantics
  void free() {
    if (ref._tag == 3) {
      calloc.free(ref._payload.string);
    }
    calloc.free(this);
  }
}

class DartValueFields extends Union {
  /// [Pointer] to some Rust object.
  external Pointer ptr;

  /// [Pointer] to a [Handle] to some Dart object.
  external Pointer<Handle> handlePtr;

  /// [Pointer] to native string.
  external Pointer<Utf8> string;

  /// Numeric value.
  @Int64()
  external int number;
}
