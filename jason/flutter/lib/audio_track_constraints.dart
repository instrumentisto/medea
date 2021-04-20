import 'dart:ffi';
import 'package:ffi/ffi.dart';

import 'jason.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';

typedef _new_C = Pointer Function();
typedef _new_Dart = Pointer Function();

typedef _deviceId_C = Void Function(Pointer, Pointer<Utf8>);
typedef _deviceId_Dart = void Function(Pointer, Pointer<Utf8>);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _new = dl.lookupFunction<_new_C, _new_Dart>('AudioTrackConstraints__new');

final _deviceId = dl.lookupFunction<_deviceId_C, _deviceId_Dart>(
    'AudioTrackConstraints__device_id');

final _free =
    dl.lookupFunction<_free_C, _free_Dart>('AudioTrackConstraints__free');

class AudioTrackConstraints {
  final NullablePointer ptr = NullablePointer(_new());

  void deviceId(String deviceId) {
    var deviceIdPtr = deviceId.toNativeUtf8();
    try {
      _deviceId(ptr.getInnerPtr(), deviceIdPtr);
    } finally {
      calloc.free(deviceIdPtr);
    }
  }

  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
