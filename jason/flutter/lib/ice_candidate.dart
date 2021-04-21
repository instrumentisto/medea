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

Pointer<Utf8> candidate(Object iceCandidate) {
  if (iceCandidate is RTCIceCandidate) {
    return Utf8.toUtf8(iceCandidate.candidate);
  } else {
    throw Exception("Unknown object provided from Rust side: " + iceCandidate.runtimeType.toString());
  }
}

int sdpMLineIndex(Object iceCandidate) {
  if (iceCandidate is RTCIceCandidate) {
    return iceCandidate.sdpMlineIndex;
  } else {
    throw Exception("Unknown object provided from Rust side: " + iceCandidate.runtimeType.toString());
  }
}

Pointer<Utf8> sdpMid(Object iceCandidate) {
  if (iceCandidate is RTCIceCandidate) {
    return Utf8.toUtf8(iceCandidate.sdpMid);
  } else {
    throw Exception("Unknown object provided from Rust side: " + iceCandidate.runtimeType.toString());
  }
}