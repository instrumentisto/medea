import 'dart:ffi';

class HandleOption {
  Handle _some;
  bool _isSome;

  Option.some(Object val) {
    _some = val;
    _isSome = true;
  }

  Option.none() {
    _isSome = false;
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
      return null;
    }
  }
}

class RustStringOption extends Struct {
  @Int8()
  external int _is_some;
  external Pointer<Utf8> _val;

  RustStringOption.some(String val) {
    _is_some = 1;
    _val = val.toNativeString();
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