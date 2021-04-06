import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';

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
      "register_any_closure_caller"
  )(Pointer.fromFunction<Void Function(Handle, Pointer)>(doPointerClosureCallback));
}

final _get_remote_member_id_Dart _get_remote_member_id = _dl.lookupFunction<_get_remote_member_id_C, _get_remote_member_id_Dart>('ConnectionHandle__get_remote_member_id');
typedef _get_remote_member_id_C = Pointer<Utf8> Function(Pointer);
typedef _get_remote_member_id_Dart = Pointer<Utf8> Function(Pointer);

void doClosureCallback(void Function() callback) {
  callback();
}

void doPointerClosureCallback(void Function(Pointer) callback, Pointer pointer) {
  callback(pointer);
}

final cb_test = _dl.lookupFunction<
    Void Function(Handle),
    void Function(void Function(Pointer))>("cb_test");

void simpleCallback() {
  doDynamicLinking();
  cb_test((conn) {
      var str = _get_remote_member_id(conn).toDartString();
      print('callback fired: $str');
  });
}
