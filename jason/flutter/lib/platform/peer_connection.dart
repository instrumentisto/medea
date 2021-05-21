import 'package:flutter/cupertino.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'utils/option.dart';
import 'utils/array.dart';
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_RtcPeerConnection__set_remote_description')(
      Pointer.fromFunction<
          Handle Function(
              Handle, Pointer<Utf8>, Pointer<Utf8>)>(setRemoteDescription));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_RtcPeerConnection__set_local_description')(
      Pointer.fromFunction<
          Handle Function(
              Handle, Pointer<Utf8>, Pointer<Utf8>)>(setRemoteDescription));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_RtcPeerConnection__add_ice_candidate')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(addIceCandidate));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_RtcPeerConnection__ice_connection_state')(
      Pointer.fromFunction<RustIntOption Function(Handle)>(iceConnectionState));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_RtcPeerConnection__connection_state')(
      Pointer.fromFunction<RustIntOption Function(Handle)>(connectionState));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_RtcPeerConnection__restart_ice')(
      Pointer.fromFunction<Void Function(Handle)>(restartIce));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_RtcPeerConnection__rollback')(
      Pointer.fromFunction<Void Function(Handle)>(rollback));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_RtcPeerConnection__on_track')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(onTrack));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_RtcPeerConnection__on_ice_candidate')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(onIceCandidate));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_RtcPeerConnection__on_ice_connection_state_change')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(
          onIceConnectionStateChange));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_RtcPeerConnection__on_connection_state_change')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(
          onConnectionStateChange));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_RtcPeerConnection__new_peer')(
      Pointer.fromFunction<Handle Function()>(newPeer));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_RtcPeerConnection__add_transceiver')(
      Pointer.fromFunction<Handle Function(Handle, Int32, Int32)>(
          addTransceiver));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_RtcPeerConnection__get_transceiver_by_mid')(
      Pointer.fromFunction<Handle Function(Handle, Pointer<Utf8>)>(
          getTransceiverByMid));
}

Object addTransceiver(Object peer, int kind, int direction) async {
  try {
    peer = peer as RTCPeerConnection;

    try {
      var fut = await peer.addTransceiver(
        kind: RTCRtpMediaType.values[kind],
        init: RTCRtpTransceiverInit(
            direction: TransceiverDirection.values[direction]),
      );
      return fut;
    } catch (e, stacktrace) {
      throw e;
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

Object newPeer() {
  try {
    return createPeerConnection({
      'iceServers': [
        {'url': 'stun:stun.l.google.com:19302'},
      ],
      'sdpSemantics': 'unified-plan'
    });
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

void onTrack(Object conn, Object f) {
  try {
    if (conn is RTCPeerConnection) {
      if (f is Function) {
        conn.onTrack = (e) {
          f(e.track);
        };
      }
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

void onIceCandidate(Object conn, Object f) {
  try {
    if (conn is RTCPeerConnection) {
      if (f is Function) {
        conn.onIceCandidate = (e) {
          f(e);
        };
      }
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

void onIceConnectionStateChange(Object conn, Object f) {
  try {
    if (conn is RTCPeerConnection) {
      if (f is Function) {
        conn.onIceConnectionState = (e) {
          f(e.index);
        };
      }
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

void onConnectionStateChange(Object conn, Object f) {
  try {
    if (conn is RTCPeerConnection) {
      if (f is Function) {
        conn.onConnectionState = (e) {
          f(e.index);
        };
      }
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

// TODO: DartOption Future
Object getTransceiverByMid(Object peer, Pointer<Utf8> mid) async {
  try {
    peer = peer as RTCPeerConnection;
    var transceivers = await peer.getTransceivers();
    var mMid = mid.toDartString();
    for (var transceiver in transceivers) {
      if (transceiver.mid == mMid) {
        return transceiver;
      }
    }
    throw Exception("Transceiver not found!!!!");
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

Future<RTCRtpTransceiver> foobar(RTCPeerConnection peer, String mid) async {
  try {
    var transceivers = await peer.getTransceivers();
    for (var transceiver in transceivers) {
      if (transceiver.mid == mid) {
        return transceiver;
      }
    }
    throw Exception("Transceiver not found!!!!");
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

Object setRemoteDescription(
    Object conn, Pointer<Utf8> type, Pointer<Utf8> sdp) {
  try {
    print("setRemoteDescription");
    conn = conn as RTCPeerConnection;
    print("Peer casted");
    var desc = RTCSessionDescription(sdp.toDartString(), type.toDartString());
    print("Desc created");
    return conn.setRemoteDescription(desc);
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

Object setLocalDescription(Object conn, Pointer<Utf8> type, Pointer<Utf8> sdp) {
  try {
    conn = conn as RTCPeerConnection;
    return conn.setLocalDescription(
        RTCSessionDescription(sdp.toDartString(), type.toDartString()));
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

void restartIce(Object conn) {
  try {
    if (conn is RTCPeerConnection) {
      throw Exception("Unimplemented");
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

// TODO: Return Future to Rust
void addIceCandidate(Object conn, Object candidate) {
  try {
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
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

RustIntOption connectionState(Object conn) {
  try {
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
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

RustIntOption iceConnectionState(Object conn) {
  try {
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
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

void rollback(Object conn) {
  try {
    if (conn is RTCPeerConnection) {
      conn.setLocalDescription(RTCSessionDescription(null, "rollback"));
    } else {
      throw Exception(
          "Unexpected Object received: " + conn.runtimeType.toString());
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

Object getTransceivers(Object conn) {
  try {
    if (conn is RTCPeerConnection) {
      return conn.getTransceivers();
    } else {
      throw Exception(
          "Unexpected Object received: " + conn.runtimeType.toString());
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}
