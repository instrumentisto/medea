import 'dart:ffi';
import 'package:ffi/ffi.dart';

class Result<T> {
  T? _ok;
  JasonError? _err;

  Result.ok(T res) {
    _ok = res;
  }

  Result.err(JasonError err) {
    _err = err;
  }

  T unwrap() {
    if (_err == null) {
      throw Exception(_err);
    } else {
      return _ok!;
    }
  }
}

class JasonError {
  late String _name;
  late String _message;
  late String _stacktrace;
  Object? _source;

  JasonError.withoutSource(
      Pointer<Utf8> name, Pointer<Utf8> message, Pointer<Utf8> stacktrace) {
    _name = name.toDartString();
    _message = message.toDartString();
    _stacktrace = stacktrace.toDartString();
  }

  JasonError.withSource(Pointer<Utf8> name, Pointer<Utf8> message,
      Pointer<Utf8> stacktrace, Object source) {
    _name = name.toDartString();
    _message = message.toDartString();
    _stacktrace = stacktrace.toDartString();
    _source = source;
  }

  String getName() {
    return _name;
  }

  String getMessage() {
    return _message;
  }

  String getStacktrace() {
    return _stacktrace;
  }

  Object? getSource() {
    return _source;
  }
}
