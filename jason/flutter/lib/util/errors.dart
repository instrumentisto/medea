import 'dart:ffi';

class NullNativePointerException implements Exception {
  @override
  String toString() {
    return 'Pointer is null';
  }
}


// TODO: meh, not gonna work, we need some Pointer wrapper
void assertNonNull(Pointer ptr) {
  if (ptr.address == 0) {
    throw NullNativePointerException();
  }
}
