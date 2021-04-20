import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'ffi.dart' as ffi;
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions() {
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_PeerConnection__set_remote_description')(
      Pointer.fromFunction<Handle Function(Handle)>(setRemoteDescription)
  );
}

void setRemoteDescription(Object conn, Pointer<Utf8> sdp, Pointer<Utf8> type) {
  if (conn is RTCPeerConnection) {
    conn.setRemoteDescription(RTCSessionDescription(sdp.toDartString(), type.toDartString()));
  }
}

void setLocalDescription(Object conn, Pointer<Utf8> sdp, Pointer<Utf8> type) {
  if (conn is RTCPeerConnection) {
    conn.setLocalDescription(RTCSessionDescription(sdp.toDartString(), type.toDartString()));
  }
}

int connectionState(Object conn) {
  if (conn is RTCPeerConnection) {
    return conn.connectionState.index;
  } else {
    throw Exception("Unexpected Object received: " + conn.runtimeType.toString());
  }
}

int iceConnectionState(Object conn) {
  if (conn is RTCPeerConnection) {
    return conn.iceConnectionState.index;
  } else {
    throw Exception("Unexpected Object received: " + conn.runtimeType.toString());
  }
}

