import 'dart:ffi';

class NullablePointer {
  Pointer? _ptr;

  NullablePointer(Pointer ptr) {
    if (ptr.address == 0) {
      throw ArgumentError.notNull('ptr');
    }
    _ptr = ptr;
  }

  Pointer getInnerPtr() {
    if (_ptr == null) {
      throw StateError('NullablePointer cannot be used after free.');
    } else {
      return Pointer.fromAddress(_ptr!.address);
    }
  }

  void free() {
    _ptr = null;
  }
}
