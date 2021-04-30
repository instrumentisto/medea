import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:medea_jason/option.dart';
import 'array.dart';
import 'array.dart';
import 'ffi.dart' as ffi;
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions() {
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_PeerConnection__set_remote_description')(
      Pointer.fromFunction<
          Handle Function(
              Handle, Pointer<Utf8>, Pointer<Utf8>)>(setRemoteDescription));
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_PeerConnection__set_local_description')(
      Pointer.fromFunction<
          Handle Function(
              Handle, Pointer<Utf8>, Pointer<Utf8>)>(setRemoteDescription));

  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_PeerConnection__add_ice_candidate')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(addIceCandidate));

  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_PeerConnection__ice_connection_state')(
      Pointer.fromFunction<RustIntOption Function(Handle)>(iceConnectionState));

  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_PeerConnection__connection_state')(
      Pointer.fromFunction<RustIntOption Function(Handle)>(connectionState));

  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_PeerConnection__restart_ice')(
      Pointer.fromFunction<Void Function(Handle)>(restartIce));

  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_PeerConnection__rollback')(
      Pointer.fromFunction<Void Function(Handle)>(rollback));

  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_PeerConnection__on_track')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(onTrack));
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_PeerConnection__on_ice_candidate')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(onIceCandidate));
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_PeerConnection__on_ice_connection_state_change')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(
          onIceConnectionStateChange));
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_PeerConnection__on_connection_state_change')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(
          onConnectionStateChange));
}

void onTrack(Object conn, Object f) {
  if (conn is RTCPeerConnection) {
    if (f is Function) {
      conn.onTrack = (e) {
        f(e.track);
      };
    }
  }
}

void onIceCandidate(Object conn, Object f) {
  if (conn is RTCPeerConnection) {
    if (f is Function) {
      conn.onIceCandidate = (e) {
        f(e);
      };
    }
  }
}

void onIceConnectionStateChange(Object conn, Object f) {
  if (conn is RTCPeerConnection) {
    if (f is Function) {
      conn.onIceConnectionState = (e) {
        f(e.index);
      };
    }
  }
}

void onConnectionStateChange(Object conn, Object f) {
  if (conn is RTCPeerConnection) {
    if (f is Function) {
      conn.onConnectionState = (e) {
        f(e.index);
      };
    }
  }
}

Object setRemoteDescription(
    Object conn, Pointer<Utf8> sdp, Pointer<Utf8> type) {
  conn = conn as RTCPeerConnection;
  return conn.setRemoteDescription(
      RTCSessionDescription(sdp.toDartString(), type.toDartString()));
}

Object setLocalDescription(Object conn, Pointer<Utf8> sdp, Pointer<Utf8> type) {
  conn = conn as RTCPeerConnection;
  return conn.setLocalDescription(
      RTCSessionDescription(sdp.toDartString(), type.toDartString()));
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
      throw Exception(
          "Unexpected Object received: " + candidate.runtimeType.toString());
    }
  } else {
    throw Exception(
        "Unexpected Object received: " + candidate.runtimeType.toString());
  }
}

RustIntOption connectionState(Object conn) {
  if (conn is RTCPeerConnection) {
    if (conn.connectionState != null) {
      return RustIntOption.some(conn.connectionState!.index);
    } else {
      return RustIntOption.none();
    }
  } else {
    throw Exception(
        "Unexpected Object received: " + conn.runtimeType.toString());
  }
}

RustIntOption iceConnectionState(Object conn) {
  if (conn is RTCPeerConnection) {
    if (conn.iceConnectionState != null) {
      return RustIntOption.some(conn.iceConnectionState!.index);
    } else {
      return RustIntOption.none();
    }
  } else {
    throw Exception(
        "Unexpected Object received: " + conn.runtimeType.toString());
  }
}

void rollback(Object conn) {
  if (conn is RTCPeerConnection) {
    conn.setLocalDescription(RTCSessionDescription(null, "rollback"));
  } else {
    throw Exception(
        "Unexpected Object received: " + conn.runtimeType.toString());
  }
}

Object getTransceivers(Object conn) {
  if (conn is RTCPeerConnection) {
    return conn.getTransceivers();
  } else {
    throw Exception(
        "Unexpected Object received: " + conn.runtimeType.toString());
  }
}
