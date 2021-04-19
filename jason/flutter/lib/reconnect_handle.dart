import 'dart:ffi';

import 'jason.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _free_Dart _free =
    dl.lookupFunction<_free_C, _free_Dart>('ReconnectHandle__free');

class ReconnectHandle {
  late NullablePointer ptr;

  ReconnectHandle(this.ptr);

  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
