library jason;

import 'dart:ffi';
import 'ffi.dart' as ffi;
import 'package:ffi/ffi.dart';
import 'executor.dart';

class Array extends Struct {
  @Uint64()
  external int len;
  external Pointer<Pointer> arr;
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

class Jason {
  late Executor _executor;

  Jason() {
    ffi.doDynamicLinking();
    _executor = new Executor(ffi.dl);
    _executor.start();
  }

  void cb_test() {
    ffi.simpleCallback();
  }

  Future<void> foobar() async {
    await ffi.foobar();
  }
}
