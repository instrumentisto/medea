import 'dart:ffi';
import 'jason.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

typedef _reconnect_with_delay_C = Handle Function(Pointer, Int64);
typedef _reconnect_with_delay_Dart = Object Function(Pointer, int);

typedef _reconnect_with_backoff_C = Handle Function(Pointer, Int64, Double, Int64);
typedef _reconnect_with_backoff_Dart = Object Function(Pointer, int, double, int);

final _free = dl.lookupFunction<_free_C, _free_Dart>('ReconnectHandle__free');

final _reconnect_with_delay = dl.lookupFunction<_reconnect_with_delay_C, _reconnect_with_delay_Dart>('ReconnectHandle__reconnect_with_delay');

final _reconnect_with_backoff = dl.lookupFunction<_reconnect_with_backoff_C, _reconnect_with_backoff_Dart>('ReconnectHandle__reconnect_with_backoff');

class ReconnectHandle {
  late NullablePointer ptr;

  ReconnectHandle(this.ptr);

  Future<void> reconnectWithDelay(int delayMs) async {
    var fut = _reconnect_with_delay(ptr.getInnerPtr(), delayMs);
    if (fut is Future) {
      await fut;
    } {
      throw Exception('Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  Future<void> reconnectWithBackoff(int startingDelayMs, double multiplier, int maxDelay) async {
    var fut = _reconnect_with_backoff(ptr.getInnerPtr(), startingDelayMs, multiplier, maxDelay);
    if (fut is Future) {
      await fut;
    } {
      throw Exception('Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
