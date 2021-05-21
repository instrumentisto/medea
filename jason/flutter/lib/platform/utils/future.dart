import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'package:medea_jason/jason.dart';
import 'package:medea_jason/platform/utils/option.dart';

typedef _resolveOk_C = Void Function(Pointer, Handle);
typedef _resolveOk_Dart = void Function(Pointer, Object);

typedef _resolveErr_C = Void Function(Pointer, Handle);
typedef _resolveErr_Dart = void Function(Pointer, Object);

typedef _resolveVoid_C = Void Function(Pointer);
typedef _resolveVoid_Dart = void Function(Pointer);

typedef _resolveHandleOption_C = Void Function(Pointer, Handle);
typedef _resolveHandleOption_Dart = void Function(Pointer, Object);

final _resolveOk =
    dl.lookupFunction<_resolveOk_C, _resolveOk_Dart>('DartFuture__resolve_ok');
final _resolveErr = dl
    .lookupFunction<_resolveErr_C, _resolveErr_Dart>('DartFuture__resolve_err');
final _resolveVoid = dl.lookupFunction<_resolveVoid_C, _resolveVoid_Dart>(
    'VoidDartFuture__resolve');
final _resolveHandleOption =
    dl.lookupFunction<_resolveHandleOption_C, _resolveHandleOption_Dart>(
        'DartHandleOption__resolve');

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_spawn_dart_future_function')(
      Pointer.fromFunction<Handle Function(Handle, Pointer)>(spawner));
  //register_void_future_spawner_function
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_void_future_spawner_function')(
      Pointer.fromFunction<Handle Function(Handle, Pointer)>(voidSpawner));
}

void handleOptionSpawner(Object fut, Pointer resolver) {
  try {
    fut = fut as Future;
    fut.then((value) {
      _resolveHandleOption(resolver, value);
    }, onError: (error, stackTrace) {
      print("Thrown: " + error.toString());
    });
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

void voidSpawner(Object fut, Pointer resolver) {
  try {
    if (fut is Future) {
      fut.then((val) {
        _resolveVoid(resolver);
      }, onError: (error, stackTrace) {
        print("VoidFuture thrown exception: " + error.toString());
      });
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

void spawner(Object fut, Pointer resolver) {
  try {
    if (fut is Future) {
      fut.then((val) {
        _resolveOk(resolver, val);
      }, onError: (e) {
        print("AKDJASLKDJSALKDJASKDJLSDKA");
        print("Thrown: " + e.toString());
        _resolveErr(resolver, e);
      });
    } else {
      throw Exception(
          "Unexpected Object provided from Rust: " + fut.runtimeType.toString());
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}
