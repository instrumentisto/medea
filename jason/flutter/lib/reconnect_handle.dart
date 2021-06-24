import 'dart:ffi';

import 'ffi/foreign_value.dart';
import 'jason.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

typedef _reconnect_with_delay_C = Handle Function(Pointer, Int64);
typedef _reconnect_with_delay_Dart = Object Function(Pointer, int);

typedef _reconnect_with_backoff_C = Handle Function(
    Pointer, Int64, Double, Int64, ForeignValue);
typedef _reconnect_with_backoff_Dart = Object Function(
    Pointer, int, double, int, ForeignValue);

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
  /// Throws `RpcClientException` if reconnect attempt fails.
  ///
  /// Throws [StateError] if the underlying [Pointer] has been freed.
  ///
  /// Converts the provided [delayMs] into an `u32`. Throws an [ArgumentError]
  /// if conversion fails.
  Future<void> reconnectWithDelay(int delayMs) async {
    await (_reconnect_with_delay(ptr.getInnerPtr(), delayMs) as Future);
  }

  /// Tries to reconnect a `Room` in a loop with a growing backoff delay.
  ///
  /// The first attempt will be performed immediately, and the second attempt
  /// will be performed after [starting_delay_ms].
  ///
  /// Delay between reconnection attempts won't be greater than [max_delay_ms].
  ///
  /// After each reconnection attempt, delay between reconnections will be
  /// multiplied by the given [multiplier] until it reaches [max_delay_ms].
  ///
  /// If [multiplier] is a negative number then it will be considered as `0.0`.
  /// This might cause a busy loop, so it's not recommended.
  ///
  /// Max elapsed time can be limited with an optional [maxElapsedTimeMs]
  /// argument.
  ///
  /// If the `Room` is already reconnecting then new reconnection attempt won't
  /// be performed. Instead, it will wait for the first reconnection attempt
  /// result and use it here.
  ///
  /// Throws `RpcClientException` if reconnect attempt fails.
  ///
  /// Throws [StateError] if the underlying [Pointer] has been freed.
  ///
  /// Converts the provided [startingDelayMs], [maxDelay] and [maxElapsedTimeMs]
  /// into an `u32`s. Throws an [ArgumentError] if any conversion fails.
  Future<void> reconnectWithBackoff(
      int startingDelayMs, double multiplier, int maxDelay,
      [int? maxElapsedTimeMs]) async {
    var maxElapsedTimeMs_arg = maxElapsedTimeMs == null
        ? ForeignValue.none()
        : ForeignValue.fromInt(maxElapsedTimeMs);

    await (_reconnect_with_backoff(ptr.getInnerPtr(), startingDelayMs,
        multiplier, maxDelay, maxElapsedTimeMs_arg.ref) as Future);
  }

  /// Drops the associated Rust struct and nulls the local [Pointer] to it.
  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
