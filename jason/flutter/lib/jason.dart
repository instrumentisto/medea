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
typedef _check_arr_C = Pointer<Pointer> Function();
typedef _check_arr_Dart = Pointer<Pointer> Function();

class Jason {
  late Pointer _ptr;

  Jason() {
    _ptr  = _init();
  }

  int add(int a) {
    return _add(a);
  }

  List<Pointer> check_arr() {
    var arr = _check_arr();
    var lastArrAddr = arr[0];
    List<Pointer> out = List.empty();

    for (var i, addr; addr != lastArrAddr; i++) {
      out.add(arr[i]);
      lastArrAddr = arr[i];
    }

    return out;
  }

  String foobar() {
    var str = _foobar(_ptr);
    var hey = str.toDartString();
    return hey;
  }
}
