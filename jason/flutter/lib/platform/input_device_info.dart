import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'utils/option.dart';
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_InputDeviceInfo__device_id')(
      Pointer.fromFunction<RustStringOption Function(Handle)>(deviceId));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_InputDeviceInfo__label')(
      Pointer.fromFunction<RustStringOption Function(Handle)>(label));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_InputDeviceInfo__group_id')(
      Pointer.fromFunction<RustStringOption Function(Handle)>(groupId));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_InputDeviceInfo__kind')(
      Pointer.fromFunction<RustStringOption Function(Handle)>(kind));
}

// TODO: can be just String, because device_id is always Some.
RustStringOption deviceId(Object deviceInfo) {
  try {
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
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

// TODO: can be just String, because label is always Some.
RustStringOption label(Object deviceInfo) {
  try {
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
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

RustStringOption groupId(Object deviceInfo) {
  try {
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
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

RustStringOption kind(Object deviceInfo) {
  try {
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
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}
