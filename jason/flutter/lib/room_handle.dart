import 'dart:ffi';

import 'package:medea_jason/util/nullable_pointer.dart';

import 'jason.dart';
import 'util/move_semantic.dart';

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _free_Dart _free =
    dl.lookupFunction<_free_C, _free_Dart>('RoomHandle__free');

class RoomHandle {
  late NullablePointer ptr;

  RoomHandle(this.ptr);

  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
