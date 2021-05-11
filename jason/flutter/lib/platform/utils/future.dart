import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'package:medea_jason/jason.dart';

typedef _resolveOk_C = Void Function(Pointer, Handle);
typedef _resolveOk_Dart = void Function(Pointer, Object);

typedef _resolveErr_C = Void Function(Pointer, Handle);
typedef _resolveErr_Dart = void Function(Pointer, Object);

typedef _resolveVoid_C = Void Function(Pointer);
typedef _resolveVoid_Dart = void Function(Pointer);

final _resolveOk = dl
    .lookupFunction<_resolveOk_C, _resolveOk_Dart>('DartFuture__resolve_ok');
final _resolveErr = dl
    .lookupFunction<_resolveErr_C, _resolveErr_Dart>('DartFuture__resolve_err');
final _resolveVoid = dl.lookupFunction<_resolveVoid_C, _resolveVoid_Dart>('VoidDartFuture__resolve');

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_spawn_dart_future_function')(
      Pointer.fromFunction<Handle Function(Handle, Pointer)>(spawner));
  //register_void_future_spawner_function
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      'register_void_future_spawner_function')(
      Pointer.fromFunction<Handle Function(Handle, Pointer)>(voidSpawner));
}

void voidSpawner(Object fut, Pointer resolver) {
  if (fut is Future) {
    fut.then((val) {
      _resolveVoid(resolver);
    });
  }
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
