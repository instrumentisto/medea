import 'package:flutter_webrtc/flutter_webrtc.dart';
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

Pointer<Utf8> deviceId(Object deviceInfo) {
  if (deviceInfo is MediaDeviceInfo) {
    return Utf8.toUtf8(deviceInfo.deviceId);
  } else {
    throw Exception("Unexpected Object provided from Rust side: " + deviceInfo.runtimeType.toString());
  }
}

Pointer<Utf8> label(Object deviceInfo) {
  if (deviceInfo is MediaDeviceInfo) {
    return Utf8.toUtf8(deviceInfo.label);
  } else {
    throw Exception("Unexpected Object provided from Rust side: " + deviceInfo.runtimeType.toString());
  }
}

Pointer<Utf8> groupId(Object deviceInfo) {
  if (deviceInfo is MediaDeviceInfo) {
    return Utf8.toUtf8(deviceInfo.groupId);
  } else {
    throw Exception("Unexpected Object provided from Rust side: " + deviceInfo.runtimeType.toString());
  }
}

Pointer<Utf8> kind(Object deviceInfo) {
  if (deviceInfo is MediaDeviceInfo) {
    return Utf8.toUtf8(deviceInfo.kind);
  } else {
    throw Exception("Unexpected Object provided from Rust side: " + deviceInfo.runtimeType.toString());
  }
}