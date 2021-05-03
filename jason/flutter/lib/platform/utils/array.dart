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

  Array(List<Pointer> list) {
    final ptr = malloc.allocate<Pointer>(sizeOf<Pointer>() * list.length);
    for (var i = 0; i < list.length; i++) {
      ptr.elementAt(i).value = list[i];
    }

    _len = list.length;
    _arr = ptr;
  }

  List<Pointer> asList() {
    List<Pointer> out = List.empty(growable: true);
    for (var i = 0; i < _len; i++) {
      out.add(_arr[i]);
    }
    _free_array(this);

    return out;
  }
}
