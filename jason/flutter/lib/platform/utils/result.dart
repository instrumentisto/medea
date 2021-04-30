import 'dart:ffi';
import 'package:ffi/ffi.dart';

class DartResult extends Struct {
  @Int8()
  external int _is_ok;
  external Pointer _ok;
  external Pointer<Utf8> _err_name;
  external Pointer<Utf8> _err_message;

  DartResult.ok(Pointer res) {
    _is_ok = 1;
    _ok = res;
  }

  DartResult.err(String name, String message) {
    _is_ok = 0;
    _ok = Pointer.fromAddress(0);
    _err_name = name.toNativeUtf8();
    _err_message = message.toNativeUtf8();
  }
}
