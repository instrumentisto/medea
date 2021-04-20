import 'package:flutter_webrtc/flutter_webrtc.dart';
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

int currentDirection(Object transceiver) {
  if (transceiver is RTCRtpTransceiver) {
    return transceiver.currentDirection.index;
  }
}

void replaceSendTrack(Object transceiver, Object track) {
  if (transceiver is RTCRtpTransceiver) {
    transceiver.sender.replaceTrack(track);
  }
}

void setSenderTrackEnabled(Object transceiver, int enabled) {
  if (transceiver is RTCRtpTransceiver) {
    transceiver.sender.track.enabled = enabled == 1;
  }
}


int isStopped(Object transceiver) {
  if (transceiver is RTCRtpTransceiver) {
    return transceiver.sender.track.muted ? 1 : 0;
  }
}