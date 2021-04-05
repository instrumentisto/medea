import 'dart:ffi';
import 'dart:io';

final DynamicLibrary _dl = _open();
final DynamicLibrary dl = _dl;
DynamicLibrary _open() {
  if (Platform.isAndroid) return DynamicLibrary.open('libjason.so');
  if (Platform.isIOS) return DynamicLibrary.executable();
  throw UnsupportedError('This platform is not supported.');
}

void doDynamicLinking() {
  final nativeInitializeApi = _dl.lookupFunction<
      IntPtr Function(Pointer<Void>),
      int Function(Pointer<Void>)>("InitDartApiDL");

  if (nativeInitializeApi(NativeApi.initializeApiDLData) != 0) {
    throw "Failed to initialize Dart API";
  }

  _dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      "register_closure_caller")(
      Pointer.fromFunction<Void Function(Handle)>(doClosureCallback));

  _dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
    "register_connection_handle_closure_caller"
  )(Pointer.fromFunction<Void Function(Handle, Pointer)>(doPointerClosureCallback));
  _dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      "register_close_reason_closure_caller"
  )(Pointer.fromFunction<Void Function(Handle, Pointer)>(doPointerClosureCallback));
  _dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      "register_reconnect_handle_closure_caller"
  )(Pointer.fromFunction<Void Function(Handle, Pointer)>(doPointerClosureCallback));
  _dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      "register_local_media_track_closure_caller"
  )(Pointer.fromFunction<Void Function(Handle, Pointer)>(doPointerClosureCallback));
}

void doClosureCallback(void Function() callback) {
  callback();
}

void doPointerClosureCallback(void Function(Pointer) callback, Pointer pointer) {
  callback(pointer);
}
