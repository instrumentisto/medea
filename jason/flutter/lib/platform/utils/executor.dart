import 'dart:ffi';
import 'dart:isolate';

typedef _rustTaskPoll = Int8 Function(Pointer task);
typedef _RustTaskPoll = int Function(Pointer task);

typedef _rustTaskDrop = Void Function(Pointer task);
typedef _RustTaskDrop = void Function(Pointer task);
typedef _postCObject = Int8 Function(Int64, Pointer<Dart_CObject>);

typedef _rustLoopInit = Void Function(
    Int64 wakePort, Pointer<NativeFunction<_postCObject>> taskPost);
typedef _RustLoopInit = void Function(
    int wakePort, Pointer<NativeFunction<_postCObject>> taskPost);

class Executor {
  final _RustTaskPoll _taskPoll;
  final _RustTaskDrop _taskDrop;
  final _RustLoopInit _loopInit;
  ReceivePort? _wakePort;

  Executor(DynamicLibrary dylib)
      : _taskPoll = dylib
            .lookup<NativeFunction<_rustTaskPoll>>('task_poll')
            .asFunction(),
        _taskDrop = dylib
            .lookup<NativeFunction<_rustTaskDrop>>('task_drop')
            .asFunction(),
        _loopInit = dylib
            .lookup<NativeFunction<_rustLoopInit>>('loop_init')
            .asFunction(),
        _wakePort = null;

  bool get started => _wakePort != null;
  bool get stopped => !started;

  void start() {
    _wakePort = ReceivePort()..listen(_pollTask);
    _loopInit(_wakePort!.sendPort.nativePort, NativeApi.postCObject);
  }

  void stop() {
    _wakePort!.close();
    _wakePort = null;
  }

  void _pollTask(dynamic message) {
    final int taskAddr = message;
    final task = Pointer.fromAddress(taskAddr);

    if (_taskPoll(task) == 0) {
      _taskDrop(task);
    }
  }
}
