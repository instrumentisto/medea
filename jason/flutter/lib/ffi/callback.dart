import 'dart:ffi';

import 'package:medea_jason/ffi/foreign_value.dart';

/// Registers the closure callers functions in Rust.
void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_fn_caller')(
      Pointer.fromFunction<Void Function(Handle, ForeignValue)>(_callFn));
}

/// Function used by Rust to call closures with single [int] argument.
void _callFn(void Function(dynamic) fn, ForeignValue value) {
  var arg = value.toDart();
  if (arg != null) {
    fn(arg);
  } else {
    (fn as void Function())();
  }
}
