import 'dart:ffi';
import 'package:ffi/ffi.dart';

import 'jason.dart';
import 'util/move_semantic.dart';
import 'util/native_string.dart';
import 'util/nullable_pointer.dart';

typedef _reason_C = Pointer<Utf8> Function(Pointer);
typedef _reason_Dart = Pointer<Utf8> Function(Pointer);

typedef _isClosedByServer_C = Int8 Function(Pointer);
typedef _isClosedByServer_Dart = int Function(Pointer);

typedef _isErr_C = Int8 Function(Pointer);
typedef _isErr_Dart = int Function(Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _reason =
    dl.lookupFunction<_reason_C, _reason_Dart>('RoomCloseReason__reason');

final _isClosedByServer =
    dl.lookupFunction<_isClosedByServer_C, _isClosedByServer_Dart>(
        'RoomCloseReason__is_closed_by_server');

final _isErr =
    dl.lookupFunction<_isErr_C, _isErr_Dart>('RoomCloseReason__is_err');

final _free = dl.lookupFunction<_free_C, _free_Dart>('RoomCloseReason__free');

/// Reason of why `Room` has been closed.
///
/// This struct is passed into `RoomHandle.onClose()` callback.
class RoomCloseReason {
  /// [Pointer] to the Rust struct that backs this object.
  late NullablePointer ptr;

  /// Constructs a new [RoomCloseReason] backed by the Rust object behind the
  /// provided [Pointer].
  RoomCloseReason(this.ptr);

  /// Returns a close reason of the `Room`.
  String reason() {
    return _reason(ptr.getInnerPtr()).nativeStringToDartString();
  }

  /// Indicates whether the `Room` was closed by server.
  bool isClosedByServer() {
    return _isClosedByServer(ptr.getInnerPtr()) > 0;
  }

  /// Indicates whether the `Room`'s close reason is considered as an error.
  bool isErr() {
    return _isErr(ptr.getInnerPtr()) > 0;
  }

  /// Drops the associated Rust object and nulls the local [Pointer] to this
  /// object.
  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
