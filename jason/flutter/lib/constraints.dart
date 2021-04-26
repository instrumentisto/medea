import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'ffi.dart' as ffi;
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions() {
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_Constraints__new')(
      Pointer.fromFunction<Handle Function()>(constructor)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_Constraints__set_audio')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(sdpMLineIndex)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_Constraints__set_video')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(sdpMLineIndex)
  );
}

Object constructor() {
  return MediaStreamConstraints();
}

void setAudio(Object cons, Object val) {
  if (cons is MediaStreamConstraints) {
    cons.audio = val;
  } else {
    throw Exception("Unexpected Object provided from Rust: " + cons.runtimeType.toString());
  }
}

void setVideo(Object cons, Object val) {
  if (cons is MediaStreamConstraints) {
    cons.video = val;
  } else {
    throw Exception("Unexpected Object provided from Rust: " + cons.runtimeType.toString());
  }
}