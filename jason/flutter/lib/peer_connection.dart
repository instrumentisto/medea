import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'array.dart';
import 'array.dart';
import 'ffi.dart' as ffi;
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions() {
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_PeerConnection__set_remote_description')(
      Pointer.fromFunction<Handle Function(Handle)>(setRemoteDescription)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_PeerConnection__set_local_description')(
      Pointer.fromFunction<Handle Function(Handle)>(setRemoteDescription)
  );

  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_PeerConnection__add_ice_candidate')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(addIceCandidate)
  );

  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_PeerConnection__ice_connection_state')(
      Pointer.fromFunction<Int32 Function(Handle)>(iceConnectionState)
  );

  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_PeerConnection__connection_state')(
      Pointer.fromFunction<Int32 Function(Handle)>(connectionState)
  );

  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_PeerConnection__restart_ice')(
      Pointer.fromFunction<Int32 Function(Handle)>(restartIce)
  );

  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_PeerConnection__rollback')(
      Pointer.fromFunction<Void Function(Handle)>(rollback)
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

void restartIce(Object conn) {
  if (conn is RTCPeerConnection) {
    throw Exception("Unimplemented");
  }
}

// TODO: Return Future to Rust
void addIceCandidate(Object conn, Object candidate) {
  if (conn is RTCPeerConnection) {
    if (candidate is RTCIceCandidate) {
      conn.addCandidate(candidate);
    } else {
      throw Exception("Unexpected Object received: " + candidate.runtimeType.toString());
    }
  } else {
    throw Exception("Unexpected Object received: " + candidate.runtimeType.toString());
  }
}

Int32 connectionState(Object conn) {
  if (conn is RTCPeerConnection) {
    return conn.connectionState.index;
  } else {
    throw Exception("Unexpected Object received: " + conn.runtimeType.toString());
  }
}

Int32 iceConnectionState(Object conn) {
  if (conn is RTCPeerConnection) {
    return conn.iceConnectionState.index;
  } else {
    throw Exception("Unexpected Object received: " + conn.runtimeType.toString());
  }
}

void rollback(Object conn) {
  if (conn is RTCPeerConnection) {
    conn.setLocalDescription(RTCSessionDescription(null, "rollback"));
  } else {
    throw Exception("Unexpected Object received: " + conn.runtimeType.toString());
  }
}

Object getTransceivers(Object conn) {
  if (conn is RTCPeerConnection) {
    return conn.getTransceivers();
  } else {
    throw Exception("Unexpected Object received: " + conn.runtimeType.toString());
  }
}