import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'utils/option.dart';
import 'utils/ffi.dart' as ffi;
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions() {
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_InputDeviceInfo__device_id')(
      Pointer.fromFunction<RustStringOption Function(Handle)>(deviceId));
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_InputDeviceInfo__label')(
      Pointer.fromFunction<RustStringOption Function(Handle)>(label));
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_InputDeviceInfo__group_id')(
      Pointer.fromFunction<RustStringOption Function(Handle)>(groupId));
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_InputDeviceInfo__kind')(
      Pointer.fromFunction<RustStringOption Function(Handle)>(kind));
}

// TODO: can be just String, because device_id is always Some.
RustStringOption deviceId(Object deviceInfo) {
  if (deviceInfo is MediaDeviceInfo) {
    if (deviceInfo.deviceId != null) {
      return RustStringOption.some(deviceInfo.deviceId);
    } else {
      return RustStringOption.none();
    }
  } else {
    throw Exception("Unexpected Object provided from Rust side: " +
        deviceInfo.runtimeType.toString());
  }
}

// TODO: can be just String, because label is always Some.
RustStringOption label(Object deviceInfo) {
  if (deviceInfo is MediaDeviceInfo) {
    if (deviceInfo.label != null) {
      return RustStringOption.some(deviceInfo.label);
    } else {
      return RustStringOption.none();
    }
  } else {
    throw Exception("Unexpected Object provided from Rust side: " +
        deviceInfo.runtimeType.toString());
  }
}

RustStringOption groupId(Object deviceInfo) {
  if (deviceInfo is MediaDeviceInfo) {
    if (deviceInfo.groupId != null) {
      return RustStringOption.some(deviceInfo.groupId!);
    } else {
      return RustStringOption.none();
    }
  } else {
    throw Exception("Unexpected Object provided from Rust side: " +
        deviceInfo.runtimeType.toString());
  }
}

RustStringOption kind(Object deviceInfo) {
  if (deviceInfo is MediaDeviceInfo) {
    if (deviceInfo.kind != null) {
      return RustStringOption.some(deviceInfo.kind!);
    } else {
      return RustStringOption.none();
    }
  } else {
    throw Exception("Unexpected Object provided from Rust side: " +
        deviceInfo.runtimeType.toString());
  }
}
