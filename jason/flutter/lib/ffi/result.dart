import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'foreign_value.dart';
import 'native_string.dart';

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

class Result extends Struct {
  /// Index of the [DartValueFields] union field. `0` goes for `Void`.
  @Uint8()
  external int _tag;

  /// Actual [ForeignValue] payload.
  external ResultFields _payload;

  /// Returns Dart representation of the underlying foreign value.
  ///
  /// Returns `null` if underlying value is `void` or `()`.
  dynamic unwrap() {
    if (_tag == 0) {
      return _payload.ok.toDart();
    } else {
      throw RustException(
          _payload.err._name.nativeStringToDartString(),
          _payload.err._message.nativeStringToDartString(),
          _payload.err._stacktrace.nativeStringToDartString());
    }
  }
}

class ResultFields extends Union {
  /// Success [ForeignValue].
  external ForeignValue ok;

  /// [Error] value.
  external Error err;
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
