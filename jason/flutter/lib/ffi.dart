import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart' as ffi;

final DynamicLibrary _dl = _open();
final DynamicLibrary dl = _dl;
DynamicLibrary _open() {
  if (Platform.isAndroid) return DynamicLibrary.open('libjason.so');
  if (Platform.isIOS) return DynamicLibrary.executable();
  throw UnsupportedError('This platform is not supported.');
}

int add(
  int a,
) {
  return _add(a);
}

final _add_Dart _add = _dl.lookupFunction<_add_C, _add_Dart>('add');

typedef _add_C = Int64 Function(
  Int64 a,
);

typedef _add_Dart = int Function(
  int a,
);
