import 'dart:ffi';

/// Registers the closure callers functions in Rust.
void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      'register_ptr_arg_fn_caller')(
      Pointer.fromFunction<Void Function(Handle, Pointer)>(_callPointerArgFn));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      'register_no_args_fn_caller')(
      Pointer.fromFunction<Void Function(Handle)>(_callNoArgsFn));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      'register_int_arg_fn_caller')(
      Pointer.fromFunction<Void Function(Handle, Int64)>(_callIntArgFn));
}

/// Function used by Rust to call closures with single [Pointer] argument.
void _callPointerArgFn(void Function(Pointer) fn, Pointer arg) {
  print("[callPointerArgFn]: 1");
  fn(arg);
  print("[callPointerArgFn]: 2");
}

/// Function used by Rust to call closures without arguments.
void _callNoArgsFn(void Function() fn) {
  fn();
}

/// Function used by Rust to call closures with single [int] argument.
void _callIntArgFn(void Function(int) fn, int arg) {
  fn(arg);
}
