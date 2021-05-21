import 'dart:ffi';
import 'package:ffi/ffi.dart';

import 'package:flutter_webrtc/flutter_webrtc.dart';

import 'utils/option.dart';

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
  try {
    print("[FLUTTER] Transceiver::set_direction called");
    transceiver = transceiver as RTCRtpTransceiver;
    return transceiver.setDirection(TransceiverDirection.values[direction]);
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

RustIntOption currentDirection(Object transceiver) {
  print("currentDirection");
  try {
    transceiver = transceiver as RTCRtpTransceiver;
    if (transceiver.currentDirection != null) {
      return RustIntOption.some(transceiver.currentDirection!.index);
    } else {
      return RustIntOption.none();
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

RustStringOption mid(Object transceiver) {
  print("mid");
  try {
    transceiver = transceiver as RTCRtpTransceiver;
    if (transceiver.mid != null) {
      return RustStringOption.some(transceiver.mid!);
    } else {
      return RustStringOption.none();
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

Object sendTrack(Object transceiver) {
  print("sendTrack");
  try {
    transceiver = transceiver as RTCRtpTransceiver;
    if (transceiver.sender.track != null) {
      return RustHandleOption.some(transceiver.sender.track!);
    } else {
      return RustHandleOption.none();
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

Object getSendTrack(Object transceiver) {
  print("getSendTrack");
  try {
    transceiver = transceiver as RTCRtpTransceiver;
    if (transceiver.sender.track != null) {
      print("TRYING SOME");
      var res = RustHandleOption.some(transceiver.sender.track!);
      print("OK SOME");
      return res;
    } else {
      print("TRYING NONE");
      var res = RustHandleOption.none();
      print("OK NONE");
      return res;
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

int hasSendTrack(Object transceiver) {
  print("hasSendTrack");
  try {
    transceiver = transceiver as RTCRtpTransceiver;
    if (transceiver.sender.track == null) {
      return 0;
    } else {
      return 1;
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

void replaceSendTrack(Object transceiver, Object track) {
  print("replaceSendTrack");
  try {
    transceiver = transceiver as RTCRtpTransceiver;
    transceiver.sender.replaceTrack(track as MediaStreamTrack);
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

void setSendTrackEnabled(Object transceiver, int enabled) {
  print("setSendTrackEnabled");
  try {
    transceiver = transceiver as RTCRtpTransceiver;
    if (transceiver.sender.track != null) {
      transceiver.sender.track!.enabled = enabled == 1;
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

void dropSender(Object transceiver) {
  print("dropSender");
  if (transceiver is RTCRtpTransceiver) {
    // TODO:
    // transceiver.sender.setTrack(null);
  }
}

RustIntOption isStopped(Object transceiver) {
  print("isStopped");
  try {
    transceiver = transceiver as RTCRtpTransceiver;
    if (transceiver.sender.track != null &&
        transceiver.sender.track!.muted != null) {
      return RustIntOption.some(transceiver.sender.track!.muted! ? 1 : 0);
    } else {
      return RustIntOption.none();
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}
