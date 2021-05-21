import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'utils/option.dart';
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__id')(
      Pointer.fromFunction<RustStringOption Function(Handle)>(id));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__device_id')(
      Pointer.fromFunction<Pointer<Utf8> Function(Handle)>(deviceId));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__facing_mode')(
      Pointer.fromFunction<Pointer<Utf8> Function(Handle)>(facingMode));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__height')(
      Pointer.fromFunction<Int32 Function(Handle)>(height, 0));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__width')(
      Pointer.fromFunction<Int32 Function(Handle)>(width, 0));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__set_enabled')(
      Pointer.fromFunction<Void Function(Handle, Int8)>(setEnabled));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__enabled')(
      Pointer.fromFunction<Int8 Function(Handle)>(enabled, 0));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__stop')(
      Pointer.fromFunction<Void Function(Handle)>(stop));
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          'register_MediaStreamTrack__on_ended')(
      Pointer.fromFunction<Void Function(Handle, Handle)>(onEnded));
}

RustStringOption id(Object track) {
  try {
    track = track as MediaStreamTrack;
    if (track.id != null) {
      return RustStringOption.some(track.id!);
    } else {
      return RustStringOption.none();
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

void onEnded(Object track, Object f) {
  try {
    if (track is MediaStreamTrack) {
      if (f is Function) {
        track.onEnded = () {
          f();
        };
      }
    }
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

Pointer<Utf8> deviceId(MediaStreamTrack track) {
  try {
    return track.getConstraints()["deviceId"].toString().toNativeUtf8();
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

Pointer<Utf8> facingMode(MediaStreamTrack track) {
  try {
    return track.getConstraints()["facingMode"].toString().toNativeUtf8();
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

int height(MediaStreamTrack track) {
  try {
    return (track.getConstraints()["height"] as int);
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

int width(MediaStreamTrack track) {
  try {
    return (track.getConstraints()["width"] as int);
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

void setEnabled(MediaStreamTrack track, int enabled) {
  try {
    track.enabled = enabled == 1;
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

void stop(MediaStreamTrack track) {
  try {
    track.stop();
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}

int enabled(MediaStreamTrack track) {
  try {
    return track.enabled ? 1 : 0;
  } catch (e) {
    print("Exception was thrown: " + e.toString());
    throw e;
  }
}
