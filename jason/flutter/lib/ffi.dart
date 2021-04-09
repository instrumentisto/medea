import 'dart:async';
import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';
import 'result.dart';

final DynamicLibrary _dl = _open();
final DynamicLibrary dl = _dl;
DynamicLibrary _open() {
  if (Platform.isAndroid) return DynamicLibrary.open('libjason.so');
  if (Platform.isIOS) return DynamicLibrary.executable();
  throw UnsupportedError('This platform is not supported.');
}

void doDynamicLinking() {
  final nativeInitializeApi = _dl.lookupFunction<IntPtr Function(Pointer<Void>),
      int Function(Pointer<Void>)>("InitDartApiDL");

  if (nativeInitializeApi(NativeApi.initializeApiDLData) != 0) {
    throw "Failed to initialize Dart API";
  }

  _dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          "register_any_closure_caller")(
      Pointer.fromFunction<Void Function(Handle, Pointer)>(
          doPointerClosureCallback));

  _dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          "register_new_completer")(
      Pointer.fromFunction<Handle Function()>(newCompleter));
  _dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          "register_completer_complete")(
      Pointer.fromFunction<Void Function(Handle, Pointer)>(completerComplete));
  _dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          "register_completer_complete_error")(
      Pointer.fromFunction<Void Function(Handle, Pointer)>(
          completerCompleteError));
  _dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          "register_completer_future")(
      Pointer.fromFunction<Handle Function(Handle)>(completerFuture));
  _dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      Pointer.fromFunction<Handle Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>, Handle)>(newErrorWithSource));
}

Object newError(
    Pointer<Utf8> name,
    Pointer<Utf8> msg,
    Pointer<Utf8> stacktrace,
    ) {
  return new Result.err(new JasonError.withoutSource(name, msg, stacktrace));
}

Object newOk(
    Pointer res,
    ) {
  return new Result.ok(res);
}

Object newErrorWithSource(
    Pointer<Utf8> name,
    Pointer<Utf8> msg,
    Pointer<Utf8> stacktrace,
    Object source,
    ) {
  return new Result.err(
      new JasonError.withSource(name, msg, stacktrace, source));
}

Object newCompleter() {
  return new Completer();
}

Object completerFuture(Object completer) {
  if (completer is Completer) {
    return completer.future;
  } else {
    throw Exception("Unexpected type");
  }
}

void completerComplete(Object completer, Pointer arg) {
  if (completer is Completer) {
    completer.complete(arg);
  }
}

void completerCompleteError(Object completer, Pointer arg) {
  if (completer is Completer) {
    completer.completeError(arg);
  }
}

final _get_remote_member_id_Dart _get_remote_member_id =
    _dl.lookupFunction<_get_remote_member_id_C, _get_remote_member_id_Dart>(
        'ConnectionHandle__get_remote_member_id');
typedef _get_remote_member_id_C = Pointer<Utf8> Function(Pointer);
typedef _get_remote_member_id_Dart = Pointer<Utf8> Function(Pointer);

final _test_future_Dart _test_future =
    _dl.lookupFunction<_test_future_C, _test_future_Dart>('test_future');
typedef _test_future_C = Handle Function();
typedef _test_future_Dart = Object Function();

Future<void> foobar() async {
  await _test_future();
  print("Future resolved");
}

void doClosureCallback(void Function() callback) {
  callback();
}

void doPointerClosureCallback(
    void Function(Pointer) callback, Pointer pointer) {
  callback(pointer);
}

final cb_test = _dl.lookupFunction<Void Function(Handle),
    void Function(void Function(Pointer))>("cb_test");

void simpleCallback() {
  doDynamicLinking();
  cb_test((conn) {
    var str = _get_remote_member_id(conn).toDartString();
    print('callback fired: $str');
  });
}
