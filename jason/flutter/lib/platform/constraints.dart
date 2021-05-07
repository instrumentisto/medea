import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamConstraints__new')(
      Pointer.fromFunction<Handle Function()>(constructor));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamConstraints__set_audio')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(setAudio));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamConstraints__set_video')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(setVideo));
}

Object constructor() {
  return MediaStreamConstraints();
}

void setAudio(Object cons, Object val) {
  if (cons is MediaStreamConstraints) {
    cons.audio = val;
  } else {
    throw Exception(
        "Unexpected Object provided from Rust: " + cons.runtimeType.toString());
  }
}

void setVideo(Object cons, Object val) {
  if (cons is MediaStreamConstraints) {
    cons.video = val;
  } else {
    throw Exception(
        "Unexpected Object provided from Rust: " + cons.runtimeType.toString());
  }
}
