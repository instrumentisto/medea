import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'ptrarray.dart';

/// Catcher for the [Future.catchError] which will convert [Error] received from Rust to [RustException].
///
/// If this catcher receives not [Error] object, then received object will be thrown instead of [RustException].
void futureErrorCatcher(Object err) {
  if (err is Error) {
    throw RustException(err._name.toDartString(), err._message.toDartString(),
        err._stacktrace.toDartString());
  } else {
    throw err;
  }
}

class RustException implements Exception {
  /// Name of this [RustException].
  final String _name;

  /// Message of this [RustException].
  final String _message;

  /// Stacktrace of this [RustException].
  final String _stacktrace;

  /// Constructs new [RustException] with a provided name, message and stacktrace.
  RustException(this._name, this._message, this._stacktrace);

  /// Formats this [RustException] to the human readable [String].
  @override
  String toString() {
    return "Name: '$_name'\nMessage: '$_message'\nStacktrace: $_stacktrace";
  }

  /// Returns name of this [RustException].
  String get name {
    return _name;
  }

  /// Returns message of this [RustException].
  String get message {
    return _message;
  }

  /// Returns stacktrace of this [RustException].
  String get stacktrace {
    return _stacktrace;
  }
}

/// Result of Rust function call.
class Result extends Struct {
  /// Boolean which indicates execution result.
  ///
  /// If it 0 then [Result] is successful, otherwise execution result is failure
  @Uint8()
  external int _isOk;

  /// Type of the success value.
  ///
  /// Based on this value, Dart will determine which of success values it should return.
  @Uint8()
  external int _okType;

  /// Success value for [Result] with [Pointer] type.
  external Pointer _ptrOk;

  /// Success value for [Result] with [PtrArray] type.
  external PtrArray _arrOk;

  /// Success value for [Result] with [Pointer] for [Utf8] type.
  external Pointer<Utf8> _strOk;

  /// Success value for [Result] with [int] type.
  @Int64()
  external int _intOk;

  /// Error value for [Result].
  external Error _error;

  /// Returns contained `Ok` value.
  ///
  /// If [Result] is failure, then this function will throw [RustException] with received [Error].
  dynamic unwrap() {
    if (_isOk == 1) {
      switch (_okType) {
        case 0:
          return;
        case 1:
          return _ptrOk;
        case 2:
          return _strOk.toDartString();
        case 3:
          return _arrOk;
        case 4:
          return _intOk;
      }
    } else {
      throw RustException(_error._name.toDartString(),
          _error._message.toDartString(), _error._stacktrace.toDartString());
    }
  }
}

/// Error which can be returned from the Rust side.
class Error extends Struct {
  /// Pointer to the [Utf8] name of this [Error].
  external Pointer<Utf8> _name;

  /// Pointer to the [Utf8] message of this [Error].
  external Pointer<Utf8> _message;

  /// Pointer to the [Utf8] stacktrace of this [Error].
  external Pointer<Utf8> _stacktrace;
}
