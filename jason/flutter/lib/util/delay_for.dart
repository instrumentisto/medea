import 'dart:ffi';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_delayed_future_caller')(
      Pointer.fromFunction<Handle Function(Int32)>(delayForMs));
}

Object delayForMs(int delay) {
  return Future.delayed(Duration(milliseconds: delay));
}
