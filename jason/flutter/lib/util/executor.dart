import 'dart:ffi';
import 'dart:isolate';

typedef _executorInit_C = Void Function(Int64);
typedef _executorInit_Dart = void Function(int);

typedef _executorPollTask_C = Uint8 Function(Pointer);
typedef _executorPollTask_Dart = int Function(Pointer);

typedef _executorDropTask_C = Void Function(Pointer);
typedef _executorDropTask_Dart = void Function(Pointer);

/// Executor used to drive Rust futures.
///
/// It must be instantiated before calling any `async` Rust functions.
class Executor {
  /// Pointer to a Rust function used to initialize Rust side of this
  /// [Executor].
  final _executorInit_Dart _loopInit;

  /// Pointer to a Rust function used to poll Rust futures.
  final _executorPollTask_Dart _taskPoll;

  /// Pointer to a Rust function used to drop Rust futures on completion.
  final _executorDropTask_Dart _taskDrop;

  /// [ReceivePort] used to receive commands for polling Rust futures.
  late ReceivePort _wakePort;

  /// Creates a new [Executor].
  ///
  /// Initializes Rust part of the [Executor], creates a [ReceivePort] that
  /// accepts commands to poll Rust futures.
  Executor(DynamicLibrary dylib)
      : _loopInit = dylib
            .lookup<NativeFunction<_executorInit_C>>('rust_executor_init')
            .asFunction(),
        _taskPoll = dylib
            .lookup<NativeFunction<_executorPollTask_C>>(
                'rust_executor_poll_task')
            .asFunction(),
        _taskDrop = dylib
            .lookup<NativeFunction<_executorDropTask_C>>(
                'rust_executor_drop_task')
            .asFunction() {
    _wakePort = ReceivePort()..listen(_pollTask);
    _loopInit(_wakePort.sendPort.nativePort);
  }

  /// Polls a Rust future basing on the provided [message]. Drops that future if
  /// it's completed.
  void _pollTask(dynamic message) {
    final task = Pointer.fromAddress(message);

    if (_taskPoll(task) == 0) {
      _taskDrop(task);
    }
  }
}
