library jason;

import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'ffi.dart' as ffi;

final _add_Dart _add = ffi.dl.lookupFunction<_add_C, _add_Dart>('add');
typedef _add_C = Int64 Function(
  Int64 a,
);
typedef _add_Dart = int Function(
  int a,
);

final _init_Dart _init = ffi.dl.lookupFunction<_init_C, _init_Dart>("Jason__init");
typedef _init_C = Pointer Function();
typedef _init_Dart = Pointer Function();

final _foobar_Dart _foobar = ffi.dl.lookupFunction<_foobar_C, _foobar_Dart>('Jason__foobar');
typedef _foobar_C = Pointer<Utf8> Function(Pointer);
typedef _foobar_Dart = Pointer<Utf8> Function(Pointer);

final _check_arr_Dart _check_arr = ffi.dl.lookupFunction<_check_arr_C, _check_arr_Dart>('check_arr');
typedef _check_arr_C = Array Function();
typedef _check_arr_Dart = Array Function();

final _device_id_Dart _device_id = ffi.dl.lookupFunction<_device_id_C, _device_id_Dart>('InputDeviceInfo__device_id');
typedef _device_id_C = Pointer<Utf8> Function(Pointer);
typedef _device_id_Dart = Pointer<Utf8> Function(Pointer);

final _free_array_Dart _free_array = ffi.dl.lookupFunction<_free_array_C, _free_array_Dart>('free_array');
typedef _free_array_C = Void Function(Array);
typedef _free_array_Dart = void Function(Array);

class Array extends Struct {
  @Uint64()
  external int len;
  external Pointer<Pointer> arr;
}

class Jason {
  final Pointer _ptr = _init();

  int add(int a) {
    return _add(a);
  }

  List<Pointer> check_arr() {
    var arr = _check_arr();
    List<Pointer> out = List.empty(growable: true);

    for (var i = 0; i < arr.len; i++) {
      var foo = _device_id(arr.arr[i]);
      var hey = foo.toDartString();
      print(hey);
      out.add(arr.arr[i]);
    }

    _free_array(arr);

    return out;
  }

  String foobar() {
    var str = _foobar(_ptr);
    var hey = str.toDartString();
    return hey;
  }
}
