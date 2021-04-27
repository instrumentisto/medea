import 'dart:ffi';

import 'jason.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _free = dl.lookupFunction<_free_C, _free_Dart>('ReconnectHandle__free');

class ReconnectHandle {
  /// [Pointer] to Rust struct that backs this object.
  late NullablePointer ptr;

  /// Constructs new [ReconnectHandle] backed by Rust object behind provided
  /// [Pointer].
  ReconnectHandle(this.ptr);

  /// Drops associated Rust object and nulls the local [Pointer] to this object.
  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
