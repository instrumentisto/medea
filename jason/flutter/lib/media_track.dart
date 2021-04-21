import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'ffi.dart' as ffi;
import 'dart:ffi';
import 'package:ffi/ffi.dart';

void registerFunctions() {
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_MediaStreamTrack__id')(
      Pointer.fromFunction<Pointer<Utf8> Function(Handle)>(id)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_MediaStreamTrack__device_id')(
      Pointer.fromFunction<Pointer<Utf8> Function(Handle)>(deviceId)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_MediaStreamTrack__facing_mode')(
      Pointer.fromFunction<Pointer<Utf8> Function(Handle)>(facingMode)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_MediaStreamTrack__height')(
      Pointer.fromFunction<int Function(Handle)>(height)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_MediaStreamTrack__width')(
      Pointer.fromFunction<int Function(Handle)>(width)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_MediaStreamTrack__set_enabled')(
      Pointer.fromFunction<void Function(Handle, bool)>(setEnabled)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_MediaStreamTrack__enabled')(
      Pointer.fromFunction<bool Function(Handle)>(enabled)
  );
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>('register_MediaStreamTrack__stop')(
      Pointer.fromFunction<void Function(Handle)>(stop)
  );
}

Pointer<Utf8> id(Object track) {
  if (track is MediaStreamTrack) {
    return track.id.toNativeUtf8();
  }
}

Pointer<Utf8> deviceId(MediaStreamTrack track) {
  return track.deviceId().toNativeUtf8();
}

Pointer<Utf8> facingMode(MediaStreamTrack track) {
  return track.facingMode().toNativeUtf8();
}

int height(MediaStreamTrack track) {
  return track.height();
}

int width(MediaStreamTrack track) {
  return track.width();
}

void setEnabled(MediaStreamTrack track, bool enabled) {
  track.enabled = enabled;
}

void stop(MediaStreamTrack track) {
  track.stop();
}

bool enabled(MediaStreamTrack track) {
  return track.enabled;
}



