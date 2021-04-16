import 'dart:ffi';

import 'jason.dart';
import 'util/errors.dart';
import 'util/move_semantic.dart';

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _free_Dart _free =
    dl.lookupFunction<_free_C, _free_Dart>('ReconnectHandle__free');

class ReconnectHandle {
  late Pointer ptr;

  ReconnectHandle(Pointer p) {
    assertNonNull(p);

    ptr = p;
  }

  @moveSemantics
  void free() {
    _free(ptr);
  }
}
