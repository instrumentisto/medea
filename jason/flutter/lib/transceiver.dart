import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:jason/option.dart';
import 'ffi.dart' as ffi;
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions() {
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_Transceiver__current_direction')(
      Pointer.fromFunction<int Function(Handle)>(currentDirection)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_MediaDevices__enumerate_devices')(
      Pointer.fromFunction<Handle Function()>(enumerateDevices)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_MediaDevices__enumerate_devices')(
      Pointer.fromFunction<Handle Function(Handle)>(getDisplayMedia)
  );
}

RustIntOption currentDirection(Object transceiver) {
  if (transceiver is RTCRtpTransceiver) {
    if (transceiver.currentDirection != null) {
      return RustIntOption.some(transceiver.currentDirection.index);
    } else {
      return RustIntOption.none();
    }
  }
}

void replaceSendTrack(Object transceiver, Object track) {
  if (transceiver is RTCRtpTransceiver) {
    transceiver.sender.replaceTrack(track);
  }
}

void setSendTrack(Object transceiver, int enabled) {
  if (transceiver is RTCRtpTransceiver) {
    transceiver.sender.track.enabled = enabled == 1;
  }
}

void dropSender(Object transceiver) {
  if (transceiver is RTCRtpTransceiver) {
    transceiver.sender.setTrack(null);
  }
}

RustIntOption isStopped(Object transceiver) {
  if (transceiver is RTCRtpTransceiver) {
    if (transceiver.sender.track.muted != null) {
      return RustIntOption.some(transceiver.sender.track.muted ? 1 : 0);
    } else {
      return RustIntOption.none();
    }
  }
}