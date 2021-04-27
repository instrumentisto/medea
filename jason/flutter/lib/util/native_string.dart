import 'dart:ffi';

import 'package:ffi/ffi.dart';
import 'package:medea_jason/jason.dart';

typedef _free_C = Void Function(Pointer<Utf8>);
typedef _free_Dart = void Function(Pointer<Utf8>);

/// Frees [String] returned from Rust.
final _free = dl.lookupFunction<_free_C, _free_Dart>('String_free');

extension RustStringPointer on Pointer<Utf8> {
  /// Frees an underlying native memory, so it can only be called once.
  String nativeStringToDartString() {
    try {
      return toDartString();
    } finally {
      _free(this);
    }
  }
}
