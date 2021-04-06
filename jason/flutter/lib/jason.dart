library jason;

import 'dart:ffi';
import 'ffi.dart' as ffi;
import 'package:ffi/ffi.dart';

class Array extends Struct {
  @Uint64()
  external int len;
  external Pointer<Pointer> arr;
}

class Jason {
  void cb_test() {
    ffi.simpleCallback();
  }
}
