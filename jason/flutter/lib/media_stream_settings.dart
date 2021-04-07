import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'ffi.dart' as ffi;
import 'audio_track_constraints.dart';
import 'device_video_track_constraints.dart';
import 'display_video_track_constraints.dart';

final _audioDart _audio =
    ffi.dl.lookupFunction<_audioC, _audioDart>('MediaStreamSettings__audio');
typedef _audioC = Void Function(Pointer, Pointer);
typedef _audioDart = void Function(Pointer, Pointer);

final _deviceVideoDart _deviceVideo = ffi.dl
    .lookupFunction<_deviceVideoC, _deviceVideoDart>(
        'MediaStreamSettings__device_video');
typedef _deviceVideoC = Void Function(Pointer, Pointer);
typedef _deviceVideoDart = void Function(Pointer, Pointer);

final _displayVideoDart _displayVideo = ffi.dl
    .lookupFunction<_displayVideoC, _displayVideoDart>(
        'MediaStreamSettings__display_video');
typedef _displayVideoC = Void Function(Pointer, Pointer);
typedef _displayVideoDart = void Function(Pointer, Pointer);

class MediaStreamSettings {
  late Pointer _ptr;

  MediaStreamSettings(Pointer ptr) {
    _ptr = ptr;
  }

  void audio(AudioTrackConstraints constraints) {
    _audio(_ptr, constraints.ptr);
  }

  void deviceVideo(DeviceVideoTrackConstraints constraints) {
    _deviceVideo(_ptr, constraints.ptr);
  }

  void displayVideo(DisplayVideoTrackConstraints constraints) {
    _displayVideo(_ptr, constraints.ptr);
  }
}
