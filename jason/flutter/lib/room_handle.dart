import 'dart:ffi';

class RoomHandle {
  late Pointer ptr;

  RoomHandle(Pointer p) {
    ptr = p;
  }
}
