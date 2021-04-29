import 'dart:ffi';

import 'audio_track_constraints.dart';
import 'device_video_track_constraints.dart';
import 'display_video_track_constraints.dart';
import 'jason.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';

typedef _new_C = Pointer Function();
typedef _new_Dart = Pointer Function();

typedef _audio_C = Void Function(Pointer, Pointer);
typedef _audio_Dart = void Function(Pointer, Pointer);

typedef _deviceVideo_C = Void Function(Pointer, Pointer);
typedef _deviceVideo_Dart = void Function(Pointer, Pointer);

typedef _displayVideo_C = Void Function(Pointer, Pointer);
typedef _displayVideo_Dart = void Function(Pointer, Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _new = dl.lookupFunction<_new_C, _new_Dart>('MediaStreamSettings__new');

final _audio =
    dl.lookupFunction<_audio_C, _audio_Dart>('MediaStreamSettings__audio');

final _deviceVideo = dl.lookupFunction<_deviceVideo_C, _deviceVideo_Dart>(
    'MediaStreamSettings__device_video');

final _displayVideo = dl.lookupFunction<_displayVideo_C, _displayVideo_Dart>(
    'MediaStreamSettings__display_video');

final _free =
    dl.lookupFunction<_free_C, _free_Dart>('MediaStreamSettings__free');

/// [MediaStreamConstraints][1] wrapper.
///
/// [1]: https://w3.org/TR/mediacapture-streams#dom-mediastreamconstraints
class MediaStreamSettings {
  /// [Pointer] to the Rust struct that backs this object.
  final NullablePointer ptr = NullablePointer(_new());

  /// Specifies the nature and settings of the audio `LocalMediaTrack`.
  void audio(@moveSemantics AudioTrackConstraints constraints) {
    _audio(ptr.getInnerPtr(), constraints.ptr.getInnerPtr());
    constraints.ptr.free();
  }

  /// Set constraints that will be used to obtain local video sourced from
  /// the media device.
  void deviceVideo(@moveSemantics DeviceVideoTrackConstraints constraints) {
    _deviceVideo(ptr.getInnerPtr(), constraints.ptr.getInnerPtr());
    constraints.ptr.free();
  }

  /// Set constraints that will be used to capture local video from the user's
  /// display.
  void displayVideo(@moveSemantics DisplayVideoTrackConstraints constraints) {
    _displayVideo(ptr.getInnerPtr(), constraints.ptr.getInnerPtr());
    constraints.ptr.free();
  }

  /// Drops the associated Rust object and nulls the local [Pointer] to this
  /// object.
  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
