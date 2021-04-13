library jason;

import 'dart:ffi';
import 'ffi.dart' as ffi;
import 'package:ffi/ffi.dart';
import 'executor.dart';

class Array extends Struct {
  @Uint64()
  external int len;
  external Pointer<Pointer> arr;
}

class Jason {
  late Executor _executor;

  Jason() {
    ffi.doDynamicLinking();
    // _executor = new Executor(ffi.dl);
    // _executor.start();
  }

  void cb_test() {
    ffi.simpleCallback();
  }

  // Future<void> foobar() async {
  //   await ffi.foobar();
  // }

  void anotherFoobar() {
    ffi.anotherFoobar();
  }
}
