import 'dart:ffi';

class ReconnectHandle {
  late Pointer ptr;

  ReconnectHandle(Pointer p) {
    ptr = p;
  }
}
