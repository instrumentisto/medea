import 'dart:ffi';
import 'package:ffi/ffi.dart';

import 'jason.dart';
import 'util/errors.dart';
import 'util/move_semantic.dart';

typedef _deviceId_C = Void Function(Pointer<Utf8>);
typedef _deviceId_Dart = void Function(Pointer<Utf8>);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _deviceId_Dart _deviceId = dl.lookupFunction<_deviceId_C, _deviceId_Dart>(
    'AudioTrackConstraints__device_id');

final _free_Dart _free =
    dl.lookupFunction<_free_C, _free_Dart>('AudioTrackConstraints__free');

class AudioTrackConstraints {
  late Pointer ptr;

  AudioTrackConstraints(Pointer p) {
    assertNonNull(p);

    ptr = p;
  }

  void deviceId(String deviceId) {
    assertNonNull(ptr);

    var deviceIdPtr = deviceId.toNativeUtf8();
    try {
      _deviceId(deviceIdPtr);
    } finally {
      calloc.free(deviceIdPtr);
    }
  }

  @moveSemantics
  void free() {
    _free(ptr);
  }
}
