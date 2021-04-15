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