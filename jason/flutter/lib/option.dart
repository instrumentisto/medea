import 'dart:ffi';

class Option<T> {
  T _some;
  bool _isSome;

  Option.some(T val) {
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
