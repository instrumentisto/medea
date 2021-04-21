import 'dart:ffi';

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

void _callPointerArgFn(void Function(Pointer) fn, Pointer arg) {
  fn(arg);
}

void _callNoArgsFn(void Function() fn) {
  fn();
}

void _callIntArgFn(void Function(int) fn, int arg) {
  fn(arg);
}
