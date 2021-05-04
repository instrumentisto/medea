import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'jason.dart';
import 'track_kinds.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';
import 'util/native_string.dart';

typedef _deviceId_C = Pointer<Utf8> Function(Pointer);
typedef _deviceId_Dart = Pointer<Utf8> Function(Pointer);

typedef _label_C = Pointer<Utf8> Function(Pointer);
typedef _label_Dart = Pointer<Utf8> Function(Pointer);

typedef _kind_C = Uint8 Function(Pointer);
typedef _kind_Dart = int Function(Pointer);

typedef _nativeGroupId_C = Pointer<Utf8> Function(Pointer);
typedef _nativeGroupId_Dart = Pointer<Utf8> Function(Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _nativeGroupId = dl.lookupFunction<_nativeGroupId_C, _nativeGroupId_Dart>(
    'InputDeviceInfo__group_id');

final _kind = dl.lookupFunction<_kind_C, _kind_Dart>('InputDeviceInfo__kind');

final _label =
    dl.lookupFunction<_label_C, _label_Dart>('InputDeviceInfo__label');

final _deviceId = dl
    .lookupFunction<_deviceId_C, _deviceId_Dart>('InputDeviceInfo__device_id');

final _free = dl.lookupFunction<_free_C, _free_Dart>('InputDeviceInfo__free');

/// [`MediaDeviceInfo`][1] interface.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#device-info
class InputDeviceInfo {
  /// [Pointer] to the Rust struct backing this object.
  late NullablePointer ptr;

  /// Constructs a new [InputDeviceInfo] backed by a Rust struct behind the
  /// provided [Pointer].
  InputDeviceInfo(this.ptr);

  /// Returns an unique identifier of the represented device.
  String deviceId() {
    return _deviceId(ptr.getInnerPtr()).nativeStringToDartString();
  }

  /// Returns label describing the represented device (for example "External USB
  /// Webcam").
  ///
  /// If the device has no associated label, then returns an empty string.
  String label() {
    return _label(ptr.getInnerPtr()).nativeStringToDartString();
  }

  /// Returns kind of the represented device.
  ///
  /// This representation of a [`MediaDeviceInfo`][1] is ONLY for input devices.
  ///
  /// [1]: https://w3.org/TR/mediacapture-streams/#device-info
  MediaKind kind() {
    var index = _kind(ptr.getInnerPtr());
    return MediaKind.values[index];
  }

  /// Returns a group identifier of the represented device.
  ///
  /// Two devices have the same group identifier if they belong to the same
  /// physical device. For example, the audio input and output devices
  /// representing the speaker and microphone of the same headset have the
  /// same [`groupId`][1].
  ///
  /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediadeviceinfo-groupid
  String groupId() {
    return _nativeGroupId(ptr.getInnerPtr()).nativeStringToDartString();
  }

  /// Drops the associated Rust struct and nulls the local [Pointer] to it.
  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
