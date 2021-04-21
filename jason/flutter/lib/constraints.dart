import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'ffi.dart' as ffi;
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions() {
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_IceCandidate__candidate')(
      Pointer.fromFunction<Handle Function(Handle)>(candidate)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_IceCandidate__sdp_m_line_index')(
      Pointer.fromFunction<Handle Function()>(sdpMLineIndex)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_IceCandidate__sdp_mid')(
      Pointer.fromFunction<Handle Function(Handle)>(sdpMid)
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