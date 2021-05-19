import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_RustHandleOption__get')(
      Pointer.fromFunction<Handle Function(Handle)>(get));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_RustHandleOption__is_some')(
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
  external int? is_some;
  external Pointer<Utf8>? val;

  static RustStringOption some(String value) {
    var me = calloc<RustStringOption>();
    me.ref.is_some = 1;
    me.ref.val = value.toNativeUtf8();
    return me.ref;
  }

  static RustStringOption none() {
    var me = calloc<RustStringOption>();
    me.ref.is_some = 0;
    return me.ref;
  }
}

class RustIntOption extends Struct {
  @Int8()
  external int _is_some;
  @Int32()
  external int _val;

  static some(int val) {
    var me = calloc<RustIntOption>();
    me.ref._is_some = 1;
    me.ref._val = val;
    return me.ref;
  }

  RustIntOption.none() {
    var me = calloc<RustIntOption>();
    me.ref._is_some = 0;
    me.ref._val = 0;
  }
}
