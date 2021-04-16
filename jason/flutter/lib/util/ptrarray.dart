import 'dart:ffi';

import '../jason.dart';

final _free_rust_array_Dart _free_rust_array = dl
    .lookupFunction<_free_rust_array_C, _free_rust_array_Dart>('PtrArray_free');
typedef _free_rust_array_C = Void Function(PtrArray);
typedef _free_rust_array_Dart = void Function(PtrArray);

class PtrArray extends Struct {
  @Uint64()
  external int _len;
  external Pointer<Pointer> _arr;

  List<Pointer> intoList() {
    var out = List<Pointer>.empty(growable: true);
    for (var i = 0; i < _len; i++) {
      out.add(_arr[i]);
    }
    // TODO: check that 'this' keyword is correct here
    _free_rust_array(this);

    return out;
  }
}
