import 'dart:ffi';
import 'dart:io';

typedef _add_C = Int64 Function(Int64 a, Int64 b);
typedef _add_Dart = int Function(int a, int b);

final DynamicLibrary _dl = _load();

final _add_Dart _add = _dl.lookupFunction<_add_C, _add_Dart>('add');

DynamicLibrary _load() {
  if (Platform.isAndroid) return DynamicLibrary.open('libjason.so');
  throw UnsupportedError('This platform is not supported.');
}

int add(int a, int b) {
  return _add(a, b);
}
