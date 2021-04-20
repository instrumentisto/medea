import 'dart:ffi';

import '../jason.dart';

void registerFunctions() {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_pointer_closure_caller')(
      Pointer.fromFunction<Void Function(Handle, Pointer)>(callPointerClosure));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_unit_closure_caller')(
      Pointer.fromFunction<Void Function(Handle)>(callUnitClosure));
}

void callPointerClosure(void Function(Pointer) callback, Pointer pointer) {
  callback(pointer);
}

void callUnitClosure(void Function() callback) {
  callback();
}
