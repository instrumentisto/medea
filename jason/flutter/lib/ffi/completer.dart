import 'dart:async';
import 'dart:ffi';

import 'foreign_value.dart';

/// Registers functions that allow Rust to manage [Completer]s.
void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_new_completer_caller')(
      Pointer.fromFunction<Handle Function()>(_Completer_new));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_completer_future_caller')(
      Pointer.fromFunction<Handle Function(Handle)>(_Completer_future));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_completer_complete_caller')(
      Pointer.fromFunction<Void Function(Handle, ForeignValue)>(
          _Completer_complete));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_completer_complete_error_caller')(
      Pointer.fromFunction<Void Function(Handle, Pointer)>(
          _Completer_completeError_Pointer));
}

/// Returns a new [Completer].
Object _Completer_new() {
  return Completer();
}

/// Returns a [Future] that is completed by the provided [Completer].
Object _Completer_future(Object completer) {
  return (completer as Completer).future;
}

/// Completes the provided [Completer] with the provided [ForeignValue].
void _Completer_complete(Object completer, ForeignValue arg) {
  (completer as Completer).complete(arg.toDart());
}

/// Complete the provided [Completer] with an error.
void _Completer_completeError_Pointer(Object completer, Pointer arg) {
  (completer as Completer).completeError(arg);
}
