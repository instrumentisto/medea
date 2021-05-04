import 'dart:ffi';
import '../jason.dart';

typedef _resolveOk_C = Void Function(Pointer, Handle);
typedef _resolveOk_Dart = void Function(Pointer, Object);

typedef _resolveErr_C = Void Function(Pointer, Handle);
typedef _resolveErr_Dart = void Function(Pointer, Object);

final _resolveOk =
    dl.lookupFunction<_resolveOk_C, _resolveOk_Dart>('DartFuture__resolve_ok');
final _resolveErr = dl
    .lookupFunction<_resolveErr_C, _resolveErr_Dart>('DartFuture__resolve_err');

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_future_spawner_caller')(
      Pointer.fromFunction<Handle Function(Handle, Pointer)>(spawner));
}

void spawner(Object fut, Pointer resolver) {
  fut = fut as Future;
  fut.then((val) {
    _resolveOk(resolver, val);
  }, onError: (e) {
    _resolveErr(resolver, e);
  });
}
