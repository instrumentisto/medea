import 'dart:ffi';

import '../jason.dart';

typedef _free_C = Void Function(PtrArray);
typedef _free_Dart = void Function(PtrArray);

final _free_Dart _free =
    dl.lookupFunction<_free_C, _free_Dart>('PtrArray_free');

class PtrArray extends Struct {
  @Uint64()
  external int _len;
  external Pointer<Pointer> _arr;

  List<Pointer> intoList() {
    var out = List<Pointer>.empty(growable: true);
    for (var i = 0; i < _len; i++) {
      out.add(_arr.elementAt(i));
    }
    // TODO: check that 'this' keyword is correct here
    _free(this);

    return out;
  }
}
