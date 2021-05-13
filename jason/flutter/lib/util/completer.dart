import 'dart:async';
import 'dart:ffi';

import 'ptrarray.dart';

/// Registers functions that allow Rust to manage [Completer]s.
void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_new_completer_caller')(
      Pointer.fromFunction<Handle Function()>(_Completer_new));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_completer_future_caller')(
      Pointer.fromFunction<Handle Function(Handle)>(_Completer_future));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_completer_complete_ptr_caller')(
      Pointer.fromFunction<Void Function(Handle, Pointer)>(
          _Completer_complete_Pointer));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_completer_complete_void_caller')(
      Pointer.fromFunction<Void Function(Handle)>(_Completer_complete_Void));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_completer_complete_error_caller')(
      Pointer.fromFunction<Void Function(Handle, Pointer)>(
          _Completer_completeError_Pointer));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_completer_complete_ptr_array_caller')(
      Pointer.fromFunction<Void Function(Handle, PtrArray)>(
          _Completer_complete_PtrArray));
}

/// Returns a new [Completer].
Object _Completer_new() {
  return Completer();
}

/// Returns a [Future] that is completed by the provided [Completer].
Object _Completer_future(Object completer) {
  return (completer as Completer).future;
}

/// Completes the provided [Completer] with the provided [Pointer].
void _Completer_complete_Pointer(Object completer, Pointer arg) {
  (completer as Completer).complete(arg);
}

/// Completes the provided [Completer].
void _Completer_complete_Void(Object completer) {
  (completer as Completer).complete();
}

/// Completes the provided [Completer] with the provided [PtrArray].
void _Completer_complete_PtrArray(Object completer, PtrArray arg) {
  (completer as Completer).complete(arg);
}

/// Complete the provided [Completer] with an error.
void _Completer_completeError_Pointer(Object completer, Pointer arg) {
  (completer as Completer).completeError(arg);
}
