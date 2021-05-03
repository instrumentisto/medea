import 'ffi.dart' as ffi;
import 'dart:ffi';
import 'package:ffi/ffi.dart';

typedef _resolveOk_C = Void Function(Pointer, Handle);
typedef _resolveOk_Dart = void Function(Pointer, Object);

typedef _resolveErr_C = Void Function(Pointer, Handle);
typedef _resolveErr_Dart = void Function(Pointer, Object);

final _resolveOk = ffi.dl
    .lookupFunction<_resolveOk_C, _resolveOk_Dart>('DartFuture__resolve_ok');
final _resolveErr = ffi.dl
    .lookupFunction<_resolveErr_C, _resolveErr_Dart>('DartFuture__resolve_err');

void registerFunctions() {
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_spawn_dart_future_function')(
      Pointer.fromFunction<Handle Function(Handle, Pointer)>(spawner));
}

void spawner(Object fut, Pointer resolver) {
  if (fut is Future) {
    fut.then((val) {
      _resolveOk(resolver, val);
    }, onError: (e) {
      _resolveErr(resolver, e);
    });
  } else {
    throw Exception(
        "Unexpected Object provided from Rust: " + fut.runtimeType.toString());
  }
}
