import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'ffi.dart' as ffi;

final _deviceIdDart _deviceId = ffi.dl
    .lookupFunction<_deviceIdC, _deviceIdDart>('InputDeviceInfo__device_id');
typedef _deviceIdC = Pointer<Utf8> Function(Pointer);
typedef _deviceIdDart = Pointer<Utf8> Function(Pointer);

class AudioTrackConstraints {
  late Pointer ptr;

  AudioTrackConstraints(Pointer p) {
    ptr = p;
  }

  String deviceId() {
    return _deviceId(ptr).toDartString();
  }
}
