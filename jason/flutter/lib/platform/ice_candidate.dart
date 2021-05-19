import 'dart:ffi';
import 'package:ffi/ffi.dart';

import 'package:flutter_webrtc/flutter_webrtc.dart';

import 'utils/option.dart';
import '../util/native_string.dart';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      'register_IceCandidate__new')(Pointer.fromFunction<
          Handle Function(Pointer<Utf8>, RustStringOption, RustIntOption)>(
      newRtcIceCandidate));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_IceCandidate__candidate')(
      Pointer.fromFunction<RustStringOption Function(Handle)>(candidate));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_IceCandidate__sdp_m_line_index')(
      Pointer.fromFunction<RustIntOption Function(Handle)>(sdpMLineIndex));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_IceCandidate__sdp_mid')(
      Pointer.fromFunction<RustStringOption Function(Handle)>(sdpMid));
}

RustStringOption candidate(Object iceCandidate) {
  if (iceCandidate is RTCIceCandidate) {
    if (iceCandidate.candidate != null) {
      return RustStringOption.some(iceCandidate.candidate!);
    } else {
      return RustStringOption.none();
    }
  } else {
    throw Exception("Unknown object provided from Rust side: " +
        iceCandidate.runtimeType.toString());
  }
}

RustIntOption sdpMLineIndex(Object iceCandidate) {
  if (iceCandidate is RTCIceCandidate) {
    if (iceCandidate.sdpMlineIndex != null) {
      return RustIntOption.some(iceCandidate.sdpMlineIndex!);
    } else {
      return RustIntOption.none();
    }
  } else {
    throw Exception("Unknown object provided from Rust side: " +
        iceCandidate.runtimeType.toString());
  }
}

RustStringOption sdpMid(Object iceCandidate) {
  if (iceCandidate is RTCIceCandidate) {
    if (iceCandidate.sdpMid != null) {
      return RustStringOption.some(iceCandidate.sdpMid!);
    } else {
      return RustStringOption.none();
    }
  } else {
    throw Exception("Unknown object provided from Rust side: " +
        iceCandidate.runtimeType.toString());
  }
}

Object newRtcIceCandidate(Pointer<Utf8> candidate, RustStringOption sdpMid, RustIntOption sdpMlineIndex) {
  var sdpMidArg = sdpMid.is_some == 1 ? sdpMid.val.nativeStringToDartString() : null;
  var sdpMlineIndexArg = sdpMlineIndex.is_some == 1 ? sdpMlineIndex.val : null;
  return RTCIceCandidate(candidate.toDartString(), sdpMidArg, sdpMlineIndexArg);
}
