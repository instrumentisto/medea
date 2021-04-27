import 'dart:ffi';

import '../jason.dart';

typedef _free_C = Void Function(PtrArray);
typedef _free_Dart = void Function(PtrArray);

/// Frees [PtrArray] returned from Rust.
final _free_Dart _free =
    dl.lookupFunction<_free_C, _free_Dart>('PtrArray_free');

/// Array of [Pointer]s to Rust objects.
class PtrArray extends Struct {
  /// Length of this [PtrArray].
  @Uint64()
  external int _len;

  /// [Pointer] to the first array element.
  external Pointer<Pointer> _arr;

  /// Frees an underlying native memory, so it can only be called once.
  List<Pointer> intoPointerList() {
    try {
      var out = List<Pointer>.empty(growable: true);
      for (var i = 0; i < _len; i++) {
        out.add(_arr.elementAt(i));
      }
      return out;
    } finally {
      _free(this);
    }
  }
}
