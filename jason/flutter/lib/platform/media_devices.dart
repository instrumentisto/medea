import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaDevices__get_user_media')(
      Pointer.fromFunction<Handle Function(Handle)>(getUserMedia));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaDevices__enumerate_devices')(
      Pointer.fromFunction<Handle Function()>(enumerateDevices));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaDevices__enumerate_devices')(
      Pointer.fromFunction<Handle Function(Handle)>(getDisplayMedia));
}

Object getUserMedia(Object constraints) {
  return navigator.mediaDevices
      .getUserMedia(constraints as Map<String, dynamic>);
}

Object enumerateDevices() {
  return navigator.mediaDevices.enumerateDevices();
}

Object getDisplayMedia(Object constraints) {
  return navigator.mediaDevices
      .getDisplayMedia(constraints as Map<String, dynamic>);
}
