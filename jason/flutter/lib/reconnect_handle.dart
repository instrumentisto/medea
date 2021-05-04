import 'dart:ffi';

import 'jason.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _free = dl.lookupFunction<_free_C, _free_Dart>('ReconnectHandle__free');

/// External handle used to reconnect to a media server when connection is lost.
///
/// This handle is passed to the `RoomHandle.onConnectionLoss()` callback.
class ReconnectHandle {
  /// [Pointer] to the Rust struct backing this object.
  late NullablePointer ptr;

  /// Constructs a new [ReconnectHandle] backed by the Rust struct behind the
  /// provided [Pointer].
  ReconnectHandle(this.ptr);

  /// Drops the associated Rust struct and nulls the local [Pointer] to it.
  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
