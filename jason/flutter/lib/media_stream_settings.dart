import 'dart:ffi';

import 'audio_track_constraints.dart';
import 'device_video_track_constraints.dart';
import 'display_video_track_constraints.dart';
import 'jason.dart';
import 'util/errors.dart';
import 'util/move_semantic.dart';

typedef _audio_C = Void Function(Pointer, Pointer);
typedef _audio_Dart = void Function(Pointer, Pointer);

typedef _deviceVideo_C = Void Function(Pointer, Pointer);
typedef _deviceVideo_Dart = void Function(Pointer, Pointer);

typedef _displayVideo_C = Void Function(Pointer, Pointer);
typedef _displayVideo_Dart = void Function(Pointer, Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _audio_Dart _audio =
    dl.lookupFunction<_audio_C, _audio_Dart>('MediaStreamSettings__audio');

final _deviceVideo_Dart _deviceVideo =
    dl.lookupFunction<_deviceVideo_C, _deviceVideo_Dart>(
        'MediaStreamSettings__device_video');

final _displayVideo_Dart _displayVideo =
    dl.lookupFunction<_displayVideo_C, _displayVideo_Dart>(
        'MediaStreamSettings__display_video');

final _free_Dart _free =
    dl.lookupFunction<_free_C, _free_Dart>('MediaStreamSettings__free');

class MediaStreamSettings {
  late Pointer ptr;

  MediaStreamSettings(Pointer p) {
    assertNonNull(p);

    ptr = p;
  }

  void audio(@moveSemantics AudioTrackConstraints constraints) {
    assertNonNull(ptr);
    assertNonNull(constraints.ptr);

    _audio(ptr, constraints.ptr);
  }

  void deviceVideo(@moveSemantics DeviceVideoTrackConstraints constraints) {
    assertNonNull(ptr);
    assertNonNull(constraints.ptr);

    _deviceVideo(ptr, constraints.ptr);
  }

  void displayVideo(@moveSemantics DisplayVideoTrackConstraints constraints) {
    assertNonNull(ptr);
    assertNonNull(constraints.ptr);

    _displayVideo(ptr, constraints.ptr);
  }

  @moveSemantics
  void free() {
    _free(ptr);
  }
}
