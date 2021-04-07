import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'ffi.dart' as ffi;

final _free_array_Dart _free_array =
    ffi.dl.lookupFunction<_free_array_C, _free_array_Dart>('free_array');
typedef _free_array_C = Void Function(Array);
typedef _free_array_Dart = void Function(Array);

class Array extends Struct {
  @Uint64()
  external int _len;
  external Pointer<Pointer> _arr;

  List<Pointer> asList() {
    List<Pointer> out = List.empty(growable: true);
    for (var i = 0; i < _len; i++) {
      out.add(_arr[i]);
    }
    // TODO: check that 'this' keyword is correct here
    _free_array(this);

    return out;
  }
}
