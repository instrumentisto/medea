import 'dart:ffi';

import '../jason.dart';
import '../util/move_semantic.dart';

typedef _free_C = Void Function(PtrArray);
typedef _free_Dart = void Function(PtrArray);

/// Frees [PtrArray] returned from Rust.
final _free_Dart _free =
    dl.lookupFunction<_free_C, _free_Dart>('PtrArray_free');

/// Array of [Pointer]s to Rust objects.
class PtrArray extends Struct {
  /// [Pointer] to the first array element.
  external Pointer<Pointer> _ptr;

  /// Length of this [PtrArray].
  @Uint64()
  external int _len;

  /// Converts this [PtrArray] to a Dart's [List] of [Pointer]s.
  @moveSemantics
  List<Pointer> intoPointerList() {
    try {
      var out = List<Pointer>.empty(growable: true);
      for (var i = 0; i < _len; i++) {
        out.add(_ptr.elementAt(i));
      }
      return out;
    } finally {
      _free(this);
    }
  }
}
