import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:jason/option.dart';
import 'ffi.dart' as ffi;
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions() {
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_InputDeviceInfo__device_id')(
      Pointer.fromFunction<Pointer<Utf8> Function(Handle)>(deviceId)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_InputDeviceInfo__label')(
      Pointer.fromFunction<Pointer<Utf8> Function(Handle)>(label)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_InputDeviceInfo__group_id')(
      Pointer.fromFunction<Pointer<Utf8> Function(Handle)>(groupId)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_InputDeviceInfo__kind')(
      Pointer.fromFunction<Pointer<Utf8> Function(Handle)>(kind)
  );
}

RustStringOption deviceId(Object deviceInfo) {
  if (deviceInfo is MediaDeviceInfo) {
    if (deviceInfo.deviceId != null) {
      return RustStringOption.some(deviceInfo.deviceId.toNativeUtf8());
    } else {
      return RustStringOption.none();
    }
  } else {
    throw Exception("Unexpected Object provided from Rust side: " + deviceInfo.runtimeType.toString());
  }
}

RustStringOption label(Object deviceInfo) {
  if (deviceInfo is MediaDeviceInfo) {
    if (deviceInfo.label != null) {
      return RustStringOption.some(deviceInfo.label.toNativeUtf8());
    } else {
      return RustStringOption.none()
    }
  } else {
    throw Exception("Unexpected Object provided from Rust side: " + deviceInfo.runtimeType.toString());
  }
}

RustStringOption groupId(Object deviceInfo) {
  if (deviceInfo is MediaDeviceInfo) {
    if (deviceInfo.groupId != null) {
      return RustStringOption.some(deviceInfo.groupId.toNativeUtf8());
    } else {
      return RustStringOption.none();
    }
  } else {
    throw Exception("Unexpected Object provided from Rust side: " + deviceInfo.runtimeType.toString());
  }
}

RustStringOption kind(Object deviceInfo) {
  if (deviceInfo is MediaDeviceInfo) {
    if (deviceInfo.kind != null) {
      return RustStringOption.some(deviceInfo.kind.toNativeUtf8());
    } else {
      return RustStringOption.none();
    }
  } else {
    throw Exception("Unexpected Object provided from Rust side: " + deviceInfo.runtimeType.toString());
  }
}