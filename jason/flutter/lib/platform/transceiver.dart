import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:medea_jason/platform/utils/option.dart';
import 'utils/option.dart';
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_Transceiver__current_direction')(
      Pointer.fromFunction<RustIntOption Function(Handle)>(currentDirection));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_Transceiver__replace_track')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(replaceSendTrack));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_Transceiver__get_send_track')(
      Pointer.fromFunction<Handle Function(Handle)>(getSendTrack));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_Transceiver__set_send_track_enabled')(
      Pointer.fromFunction<Handle Function(Handle, Int8)>(setSendTrackEnabled));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_Transceiver__drop_sender')(
      Pointer.fromFunction<Void Function(Handle)>(dropSender));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_Transceiver__is_stopped')(
      Pointer.fromFunction<RustIntOption Function(Handle)>(isStopped));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_Transceiver__mid')(
      Pointer.fromFunction<RustStringOption Function(Handle)>(mid));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_Transceiver__send_track')(
      Pointer.fromFunction<Handle Function(Handle)>(sendTrack));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_Transceiver__has_send_track')(
      Pointer.fromFunction<Int8 Function(Handle)>(hasSendTrack, 0));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
      'register_Transceiver__set_direction')(
      Pointer.fromFunction<Handle Function(Handle, Int32)>(setDirection));
}

Object setDirection(Object transceiver, int direction) {
  print("[FLUTTER] Transceiver::set_direction called");
  transceiver = transceiver as RTCRtpTransceiver;
  return transceiver.setDirection(TransceiverDirection.values[direction]);
}

RustIntOption currentDirection(Object transceiver) {
  transceiver = transceiver as RTCRtpTransceiver;
  if (transceiver.currentDirection != null) {
    return RustIntOption.some(transceiver.currentDirection!.index);
  } else {
    return RustIntOption.none();
  }
}

RustStringOption mid(Object transceiver) {
  transceiver = transceiver as RTCRtpTransceiver;
  if (transceiver.mid != null) {
    return RustStringOption.some(transceiver.mid!);
  } else {
    return RustStringOption.none();
  }
}

Object sendTrack(Object transceiver) {
  transceiver = transceiver as RTCRtpTransceiver;
  if (transceiver.sender.track != null) {
    return RustHandleOption.some(transceiver.sender.track!);
  } else {
    return RustHandleOption.none();
  }
}

Object getSendTrack(Object transceiver) {
  transceiver = transceiver as RTCRtpTransceiver;
  if (transceiver.sender != null) {
    return RustHandleOption.some(transceiver.sender.track!);
  } else {
    return RustHandleOption.none();
  }
}

int hasSendTrack(Object transceiver) {
  transceiver = transceiver as RTCRtpTransceiver;
  if (transceiver.sender.track == null) {
    return 0;
  } else {
    return 1;
  }
}

void replaceSendTrack(Object transceiver, Object track) {
  transceiver = transceiver as RTCRtpTransceiver;
  transceiver.sender.replaceTrack(track as MediaStreamTrack);
}

void setSendTrackEnabled(Object transceiver, int enabled) {
  transceiver = transceiver as RTCRtpTransceiver;
  if (transceiver.sender.track != null) {
    transceiver.sender.track!.enabled = enabled == 1;
  }
}

void dropSender(Object transceiver) {
  if (transceiver is RTCRtpTransceiver) {
    // TODO:
    // transceiver.sender.setTrack(null);
  }
}

RustIntOption isStopped(Object transceiver) {
  transceiver = transceiver as RTCRtpTransceiver;
  if (transceiver.sender.track != null &&
      transceiver.sender.track!.muted != null) {
    return RustIntOption.some(transceiver.sender.track!.muted! ? 1 : 0);
  } else {
    return RustIntOption.none();
  }
}
