import 'dart:ffi';

import 'package:ffi/ffi.dart';

import '../util/move_semantic.dart';
import '../util/nullable_pointer.dart';
import 'native_string.dart';
import 'unbox_handle.dart';

/// Type-erased value that can be transferred via FFI boundaries to/from Rust.
class ForeignValue extends Struct {
  /// Index of the used [_ForeignValueFields] union field.
  ///
  /// `0` goes for no value.
  @Uint8()
  external int _tag;

  /// Actual [ForeignValue] payload.
  external _ForeignValueFields _payload;

  /// Private constructor.
  ///
  /// This class is a reference backed by a native memory, so it cannot be
  /// instantiated like a normal Dart class.
  ForeignValue._();

  /// Returns Dart representation of the underlying foreign value.
  ///
  /// Returns `null` if underlying value has no value.
  dynamic toDart() {
    switch (_tag) {
      case 0:
        return;
      case 1:
        return _payload.ptr;
      case 2:
        return unboxDartHandle(_payload.handlePtr);
      case 3:
        return _payload.string.nativeStringToDartString();
      case 4:
        return _payload.number;
      default:
        throw TypeError();
    }
  }

  /// Allocates a new [ForeignValue] with no value.
  ///
  /// This can be used when calling native function with an optional argument.
  static Pointer<ForeignValue> none() {
    return calloc<ForeignValue>();
  }

  /// Allocates a new [ForeignValue] with the provided pointer to some Rust
  /// object.
  static Pointer<ForeignValue> fromPtr(NullablePointer ptr) {
    var fVal = calloc<ForeignValue>();
    fVal.ref._tag = 1;
    fVal.ref._payload.ptr = ptr.getInnerPtr();
    return fVal;
  }

  /// Allocates a new [ForeignValue] with the provided [String].
  static Pointer<ForeignValue> fromString(String str) {
    var fVal = calloc<ForeignValue>();
    fVal.ref._tag = 3;
    fVal.ref._payload.ptr = str.toNativeUtf8();
    return fVal;
  }

  /// Allocates a new [ForeignValue] with the provided [int] value.
  static Pointer<ForeignValue> fromInt(int num) {
    var fVal = calloc<ForeignValue>();
    fVal.ref._tag = 4;
    fVal.ref._payload.number = num;
    return fVal;
  }
}

extension ForeignValuePointer on Pointer<ForeignValue> {
  /// Releases the memory allocated on a native heap.
  @moveSemantics
  void free() {
    if (ref._tag == 3) {
      calloc.free(ref._payload.string);
    }
    calloc.free(this);
  }
}

/// Possible fields of a [ForeignValue].
class _ForeignValueFields extends Union {
  /// [Pointer] to some Rust object.
  external Pointer ptr;

  /// [Pointer] to a [Handle] to some Dart object.
  external Pointer<Handle> handlePtr;

  /// [Pointer] to a native string.
  external Pointer<Utf8> string;

  /// Integer value.
  @Int64()
  external int number;
}
