import 'package:flutter/material.dart';

import 'package:medea_jason/jason.dart';
import 'dart:ffi';
import 'package:ffi/ffi.dart';

typedef _voidCallbackCall_C = Void Function(Pointer);
typedef _voidCallbackCall_Dart = void Function(Pointer);
final _voidCallbackCall =
    dl.lookupFunction<_voidCallbackCall_C, _voidCallbackCall_Dart>(
        'VoidCallback__call');

typedef _stringCallbackCall_C = Void Function(Pointer, Pointer<Utf8>);
typedef _stringCallbackCall_Dart = void Function(Pointer, Pointer<Utf8>);
final _stringCallbackCall =
    dl.lookupFunction<_stringCallbackCall_C, _stringCallbackCall_Dart>(
        'StringCallback__call');

typedef _handleMutCallbackCall_C = Void Function(Pointer, Handle);
typedef _handleMutCallbackCall_Dart = void Function(Pointer, Object);
final _handleMutCallbackCall =
    dl.lookupFunction<_handleMutCallbackCall_C, _handleMutCallbackCall_Dart>(
        'HandleMutCallback__call');

typedef _handleCallbackCall_C = Void Function(Pointer, Handle);
typedef _handleCallbackCall_Dart = void Function(Pointer, Object);
final _handleCallbackCall =
    dl.lookupFunction<_handleCallbackCall_C, _handleCallbackCall_Dart>(
        'HandleCallback__call');

typedef _intCallbackCall_C = Void Function(Pointer, Int32);
typedef _intCallbackCall_Dart = void Function(Pointer, int);
final _intCallbackCall =
    dl.lookupFunction<_intCallbackCall_C, _intCallbackCall_Dart>(
        'IntHandleCallback__call');

typedef _twoArgCallbackCall_C = Void Function(Pointer, Handle, Handle);
typedef _twoArgCallbackCall_Dart = void Function(Pointer, Object, Object);
final _twoArgCallbackCall =
    dl.lookupFunction<_twoArgCallbackCall_C, _twoArgCallbackCall_Dart>(
        'TwoArgCallback__call');

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_VoidCallback__callback')(
      Pointer.fromFunction<Handle Function(Pointer)>(voidCallback));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_StringCallback__callback')(
      Pointer.fromFunction<Handle Function(Pointer)>(stringCallback));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_HandleMutCallback__callback')(
      Pointer.fromFunction<Handle Function(Pointer)>(handleMutCallback));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_HandleCallback__callback')(
      Pointer.fromFunction<Handle Function(Pointer)>(handleCallback));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_IntCallback__callback')(
      Pointer.fromFunction<Handle Function(Pointer)>(intCallback));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_TwoArgCallback__callback')(
      Pointer.fromFunction<Handle Function(Pointer)>(twoArgCallback));
}

Object voidCallback(Pointer caller) {
  return () {
    _voidCallbackCall(caller);
  };
}

Object stringCallback(Pointer caller) {
  return (String str) {
    _stringCallbackCall(caller, str.toNativeUtf8());
  };
}

Object handleMutCallback(Pointer caller) {
  return (Object val) {
    _handleMutCallbackCall(caller, val);
  };
}

Object handleCallback(Pointer caller) {
  return (Object val) {
    _handleCallbackCall(caller, val);
  };
}

Object intCallback(Pointer caller) {
  return (int val) {
    _intCallbackCall(caller, val);
  };
}

Object twoArgCallback(Pointer caller) {
  return (Object left, Object right) {
    _twoArgCallbackCall(caller, left, right);
  };
}
