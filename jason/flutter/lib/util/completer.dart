import 'dart:async';
import 'dart:ffi';

import '../jason.dart';
import 'ptrarray.dart';

void registerFunctions() {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      'register_new_completer')(
      Pointer.fromFunction<Handle Function()>(_newCompleter));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      'register_completer_complete')(
      Pointer.fromFunction<Void Function(Handle, Pointer)>(_completerComplete));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      'register_completer_complete_error')(
      Pointer.fromFunction<Void Function(Handle, Pointer)>(_completerCompleteError));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      'register_array_completer_complete')(
      Pointer.fromFunction<Void Function(Handle, PtrArray)>(_arrayCompleterComplete));
}

Object _newCompleter() {
  return Completer();
}

void _completerComplete(Object completer, Pointer arg) {
  if (completer is Completer) {
    completer.complete(arg);
  } else {
    throw Exception('Unexpected Object received from the Rust: ' + completer.runtimeType.toString());
  }
}

void _completerCompleteError(Object completer, Pointer arg) {
  if (completer is Completer) {
    completer.completeError(arg);
  } else {
    throw Exception('Unexpected Object received from the Rust: ' + completer.runtimeType.toString());
  }
}

void _arrayCompleterComplete(Object completer, PtrArray arg) {
  if (completer is Completer) {
    completer.complete(arg);
  } else {
    throw Exception('Unexpected Object received from the Rust: ' + completer.runtimeType.toString());
  }
}
