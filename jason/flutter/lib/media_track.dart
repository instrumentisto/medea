import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'option.dart';
import 'ffi.dart' as ffi;
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions() {
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__id')(
      Pointer.fromFunction<RustStringOption Function(Handle)>(id));
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__device_id')(
      Pointer.fromFunction<Pointer<Utf8> Function(Handle)>(deviceId));
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__facing_mode')(
      Pointer.fromFunction<Pointer<Utf8> Function(Handle)>(facingMode));
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__height')(
      Pointer.fromFunction<Int32 Function(Handle)>(height, 0));
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__width')(
      Pointer.fromFunction<Int32 Function(Handle)>(width, 0));
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__set_enabled')(
      Pointer.fromFunction<Void Function(Handle, Int8)>(setEnabled));
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__enabled')(
      Pointer.fromFunction<Int8 Function(Handle)>(enabled, 0));
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__stop')(
      Pointer.fromFunction<Void Function(Handle)>(stop));
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__on_ended')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(onEnded));
}

RustStringOption id(Object track) {
  track = track as MediaStreamTrack;
  if (track.id != null) {
    return RustStringOption.some(track.id!);
  } else {
    return RustStringOption.none();
  }
}

void onEnded(Object track, Object f) {
  if (track is MediaStreamTrack) {
    if (f is Function) {
      track.onEnded = () {
        f();
      };
    }
  }
}

Pointer<Utf8> deviceId(MediaStreamTrack track) {
  return track.getConstraints()["deviceId"].toString().toNativeUtf8();
}

Pointer<Utf8> facingMode(MediaStreamTrack track) {
  return track.getConstraints()["facingMode"].toString().toNativeUtf8();
}

int height(MediaStreamTrack track) {
  return (track.getConstraints()["height"] as int);
}

int width(MediaStreamTrack track) {
  return (track.getConstraints()["width"] as int);
}

void setEnabled(MediaStreamTrack track, int enabled) {
  track.enabled = enabled == 1;
}

void stop(MediaStreamTrack track) {
  track.stop();
}

int enabled(MediaStreamTrack track) {
  return track.enabled ? 1 : 0;
}
