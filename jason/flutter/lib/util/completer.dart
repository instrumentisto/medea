import 'dart:async';
import 'dart:ffi';

import 'ptrarray.dart';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_new_completer_caller')(
      Pointer.fromFunction<Handle Function()>(_newCompleter));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_completer_complete_caller')(
      Pointer.fromFunction<Void Function(Handle, Pointer)>(_completerComplete));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_completer_complete_error_caller')(
      Pointer.fromFunction<Void Function(Handle, Pointer)>(
          _completerCompleteError));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_array_completer_complete_caller')(
      Pointer.fromFunction<Void Function(Handle, PtrArray)>(
          _arrayCompleterComplete));
}

Object _newCompleter() {
  return Completer();
}

void _completerComplete(Object completer, Pointer arg) {
  if (completer is Completer) {
    completer.complete(arg);
  } else {
    throw Exception('Unexpected Object received from the Rust: ' +
        completer.runtimeType.toString());
  }
}

void _completerCompleteError(Object completer, Pointer arg) {
  if (completer is Completer) {
    completer.completeError(arg);
  } else {
    throw Exception('Unexpected Object received from the Rust: ' +
        completer.runtimeType.toString());
  }
}

void _arrayCompleterComplete(Object completer, PtrArray arg) {
  if (completer is Completer) {
    completer.complete(arg);
  } else {
    throw Exception('Unexpected Object received from the Rust: ' +
        completer.runtimeType.toString());
  }
}
