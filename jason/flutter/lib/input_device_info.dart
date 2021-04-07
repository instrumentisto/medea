import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'ffi.dart' as ffi;
import 'kind.dart';

final _deviceIdDart _deviceId = ffi.dl
    .lookupFunction<_deviceIdC, _deviceIdDart>('InputDeviceInfo__device_id');
typedef _deviceIdC = Pointer<Utf8> Function(Pointer);
typedef _deviceIdDart = Pointer<Utf8> Function(Pointer);

final _labelDart _label =
    ffi.dl.lookupFunction<_labelC, _labelDart>('InputDeviceInfo__label');
typedef _labelC = Pointer<Utf8> Function(Pointer);
typedef _labelDart = Pointer<Utf8> Function(Pointer);

final _kindDart _kind =
    ffi.dl.lookupFunction<_kindC, _kindDart>('InputDeviceInfo__kind');
typedef _kindC = Int16 Function(Pointer);
typedef _kindDart = int Function(Pointer);

final _nativeGroupIdDart _nativeGroupId = ffi.dl
    .lookupFunction<_nativeGroupIdC, _nativeGroupIdDart>(
        'InputDeviceInfo__native_group_id');
typedef _nativeGroupIdC = Pointer<Utf8> Function(Pointer);
typedef _nativeGroupIdDart = Pointer<Utf8> Function(Pointer);

class InputDeviceInfo {
  late Pointer _ptr;

  InputDeviceInfo(Pointer ptr) {
    _ptr = ptr;
  }

  String deviceId() {
    return _deviceId(_ptr).toDartString();
  }

  String label() {
    return _label(_ptr).toDartString();
  }

  MediaKind kind() {
    return mediaKindFromInt(_kind(_ptr));
  }

  String groupId() {
    return _nativeGroupId(_ptr).toDartString();
  }
}
