import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'ffi.dart' as ffi;

void registerFunctions() {
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      'register_RustHandleOption__get')(
      Pointer.fromFunction<Handle Function(Handle)>(get));
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      'register_RustHandleOption__is_none')(
      Pointer.fromFunction<Int32 Function(Handle)>(isSome, 0));
}

Object get(Object option) {
  option = option as RustHandleOption;
  return option.some;
}

int isSome(Object option) {
  option = option as RustHandleOption;
  return option.isSome;
}

class RustHandleOption {
  Object? _some;
  late int _isSome;
  get some => _some;
  get isSome => _isSome;

  RustHandleOption.some(Object val) {
    _some = val;
    _isSome = 1;
  }

  RustHandleOption.none() {
    _isSome = 0;
  }
}

class RustOption extends Struct {
  @Int8()
  external int _is_some;
  external Pointer _val;

  Pointer some() {
    if (_is_some == 1) {
      return _val;
    } else {
      throw Exception("RustOption is None");
    }
  }
}

class RustStringOption extends Struct {
  @Int8()
  external int? _is_some;
  external Pointer<Utf8>? _val;

  RustStringOption.some(String val) {
    _is_some = 1;
    _val = val.toNativeUtf8();
  }

  RustStringOption.none() {
    _is_some = 0;
    _val = Pointer.fromAddress(0);
  }
}

class RustIntOption extends Struct {
  @Int8()
  external int _is_some;
  @Int32()
  external int _val;

  RustIntOption.some(int val) {
    _is_some = 1;
    _val = val;
  }

  RustIntOption.none() {
    _is_some = 0;
    _val = 0;
  }
}
