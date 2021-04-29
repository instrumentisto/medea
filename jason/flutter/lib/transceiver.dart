import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:jason/option.dart';
import 'ffi.dart' as ffi;
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions() {
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_Transceiver__current_direction')(
      Pointer.fromFunction<RustIntOption Function(Handle)>(currentDirection)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_Transceiver__replace_send_track')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(replaceSendTrack)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_Transceiver__get_send_track')(
      Pointer.fromFunction<Handle Function(Handle)>(getSendTrack)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_Transceiver__set_send_track_enabled')(
      Pointer.fromFunction<Handle Function(Handle, Int8)>(setSendTrackEnabled)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_Transceiver__drop_sender')(
      Pointer.fromFunction<Void Function(Handle)>(dropSender)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_Transceiver__is_stopped')(
      Pointer.fromFunction<RustIntOption Function(Handle)>(isStopped)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_Transceiver__mid')(
      Pointer.fromFunction<Pointer<Utf8> Function(Handle)>(mid)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_Transceiver__send_track')(
      Pointer.fromFunction<HandleOption Function(Handle)>(sendTrack)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_Transceiver__has_send_track')(
      Pointer.fromFunction<Int8 Function(Handle)>(hasSendTrack)
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

Pointer<Utf8> mid(Object transceiver) {
  if (transceiver is RTCRtpTransceiver) {
    return transceiver.mid.toNativeUtf8();
  }
}

HandleOption sendTrack(Object transceiver) {
  if (transceiver is RTCRtpTransceiver) {
    if (transceiver.sender.track != null) {
      return HandleOption.some(transceiver.sender.track);
    } else {
      return HandleOption.none();
    }
  }
}

int hasSendTrack(Object transceiver) {
  if (transceiver is RTCRtpTransceiver) {
    if (transceiver.sender.track == null) {
      return 0;
    } else {
      return 1;
    }
  }
}

void replaceSendTrack(Object transceiver, Object track) {
  if (transceiver is RTCRtpTransceiver) {
    transceiver.sender.replaceTrack(track);
  }
}

void setSendTrackEnabled(Object transceiver, int enabled) {
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