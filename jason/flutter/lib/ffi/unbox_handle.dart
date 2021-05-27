import 'dart:ffi';

import '../jason.dart';

typedef _unboxDartHandle_C = Handle Function(Pointer<Handle>);
typedef _unboxDartHandle_Dart = Object Function(Pointer<Handle>);

final _unboxDartHandle =
    dl.lookupFunction<_unboxDartHandle_C, _unboxDartHandle_Dart>(
        'unbox_dart_handle');

Object unboxDartHandle(Pointer<Handle> ptr) {
  return _unboxDartHandle(ptr);
}
