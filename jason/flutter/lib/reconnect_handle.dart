import 'dart:ffi';

import 'jason.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';
import 'ffi/result.dart';

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

typedef _reconnect_with_delay_C = Handle Function(Pointer, Int64);
typedef _reconnect_with_delay_Dart = Object Function(Pointer, int);

typedef _reconnect_with_backoff_C = Handle Function(
    Pointer, Int64, Double, Int64);
typedef _reconnect_with_backoff_Dart = Object Function(
    Pointer, int, double, int);

final _free = dl.lookupFunction<_free_C, _free_Dart>('ReconnectHandle__free');

final _reconnect_with_delay =
    dl.lookupFunction<_reconnect_with_delay_C, _reconnect_with_delay_Dart>(
        'ReconnectHandle__reconnect_with_delay');

final _reconnect_with_backoff =
    dl.lookupFunction<_reconnect_with_backoff_C, _reconnect_with_backoff_Dart>(
        'ReconnectHandle__reconnect_with_backoff');

/// External handle used to reconnect to a media server when connection is lost.
///
/// This handle is passed to the `RoomHandle.onConnectionLoss()` callback.
class ReconnectHandle {
  /// [Pointer] to the Rust struct backing this object.
  late NullablePointer ptr;

  /// Constructs a new [ReconnectHandle] backed by the Rust struct behind the
  /// provided [Pointer].
  ReconnectHandle(this.ptr);

  /// Tries to reconnect a `Room` after the provided delay in milliseconds.
  ///
  /// If the `Room` is already reconnecting then new reconnection attempt won't
  /// be performed. Instead, it will wait for the first reconnection attempt
  /// result and use it here.
  ///
  /// Throws [RustException] if Rust returns error.
  Future<void> reconnectWithDelay(int delayMs) async {
    await (_reconnect_with_delay(ptr.getInnerPtr(), delayMs) as Future)
        .catchError(futureErrorCatcher);
  }

  /// Tries to reconnect a `Room` in a loop with a growing backoff delay.
  ///
  /// The first attempt to reconnect is guaranteed to happen not earlier than
  /// [starting_delay_ms].
  ///
  /// Also, it guarantees that delay between reconnection attempts won't be
  /// greater than [max_delay_ms].
  ///
  /// After each reconnection attempt, delay between reconnections will be
  /// multiplied by the given [multiplier] until it reaches [max_delay_ms].
  ///
  /// If the `Room` is already reconnecting then new reconnection attempt won't
  /// be performed. Instead, it will wait for the first reconnection attempt
  /// result and use it here.
  ///
  /// If [multiplier] is negative number then [multiplier] will be considered as
  /// `0.0`.
  ///
  /// Throws [RustException] if Rust returns error.
  Future<void> reconnectWithBackoff(
      int startingDelayMs, double multiplier, int maxDelay) async {
    await (_reconnect_with_backoff(
        ptr.getInnerPtr(), startingDelayMs, multiplier, maxDelay) as Future);
  }

  /// Drops the associated Rust struct and nulls the local [Pointer] to it.
  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
