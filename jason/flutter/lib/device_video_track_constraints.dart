import 'dart:ffi';
import 'package:ffi/ffi.dart';

import 'jason.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';

typedef _new_C = Pointer Function();
typedef _new_Dart = Pointer Function();

typedef _deviceId_C = Void Function(Pointer, Pointer<Utf8>);
typedef _deviceId_Dart = void Function(Pointer, Pointer<Utf8>);

typedef _exactFacingMode_C = Void Function(Pointer, Uint8);
typedef _exactFacingMode_Dart = void Function(Pointer, int);

typedef _idealFacingMode_C = Void Function(Pointer, Uint8);
typedef _idealFacingMode_Dart = void Function(Pointer, int);

typedef _exactHeight_C = Void Function(Pointer, Uint32);
typedef _exactHeight_Dart = void Function(Pointer, int);

typedef _idealHeight_C = Void Function(Pointer, Uint32);
typedef _idealHeight_Dart = void Function(Pointer, int);

typedef _heightInRange_C = Void Function(Pointer, Uint32, Uint32);
typedef _heightInRange_Dart = void Function(Pointer, int, int);

typedef _exactWidth_C = Void Function(Pointer, Uint32);
typedef _exactWidth_Dart = void Function(Pointer, int);

typedef _idealWidth_C = Void Function(Pointer, Uint32);
typedef _idealWidth_Dart = void Function(Pointer, int);

typedef _widthInRange_C = Void Function(Pointer, Uint32, Uint32);
typedef _widthInRange_Dart = void Function(Pointer, int, int);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _new =
    dl.lookupFunction<_new_C, _new_Dart>('DeviceVideoTrackConstraints__new');

final _deviceId = dl.lookupFunction<_deviceId_C, _deviceId_Dart>(
    'DeviceVideoTrackConstraints__device_id');

final _exactFacingMode =
    dl.lookupFunction<_exactFacingMode_C, _exactFacingMode_Dart>(
        'DeviceVideoTrackConstraints__exact_facing_mode');

final _idealFacingMode =
    dl.lookupFunction<_idealFacingMode_C, _idealFacingMode_Dart>(
        'DeviceVideoTrackConstraints__ideal_facing_mode');

final _exactHeight = dl.lookupFunction<_exactHeight_C, _exactHeight_Dart>(
    'DeviceVideoTrackConstraints__exact_height');

final _idealHeight = dl.lookupFunction<_idealHeight_C, _idealHeight_Dart>(
    'DeviceVideoTrackConstraints__ideal_height');

final _heightInRange = dl.lookupFunction<_heightInRange_C, _heightInRange_Dart>(
    'DeviceVideoTrackConstraints__height_in_range');

final _exactWidth = dl.lookupFunction<_exactWidth_C, _exactWidth_Dart>(
    'DeviceVideoTrackConstraints__exact_width');

final _idealWidth = dl.lookupFunction<_idealWidth_C, _idealWidth_Dart>(
    'DeviceVideoTrackConstraints__ideal_width');

final _widthInRange = dl.lookupFunction<_widthInRange_C, _widthInRange_Dart>(
    'DeviceVideoTrackConstraints__width_in_range');

final _free =
    dl.lookupFunction<_free_C, _free_Dart>('DeviceVideoTrackConstraints__free');

/// Describes directions that a camera can face, as seen from a user's
/// perspective.
///
/// Representation of a [VideoFacingModeEnum][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams#dom-videofacingmodeenum
enum FacingMode {
  /// Facing towards a user (a self-view camera).
  User,

  /// Facing away from a user (viewing an environment).
  Environment,

  /// Facing to the left of a user.
  Left,

  /// Facing to the right of a user.
  Right,
}

/// Constraints applicable to video tracks that are sourced from some media
/// device.
class DeviceVideoTrackConstraints {
  /// [Pointer] to Rust struct that backs this object.
  final NullablePointer ptr = NullablePointer(_new());

  /// Sets exact [deviceId][1] constraint.
  ///
  /// [1]: https://w3.org/TR/mediacapture-streams#def-constraint-deviceId
  void deviceId(String deviceId) {
    var deviceIdPtr = deviceId.toNativeUtf8();
    try {
      _deviceId(ptr.getInnerPtr(), deviceIdPtr);
    } finally {
      calloc.free(deviceIdPtr);
    }
  }

  /// Sets exact [facingMode][1] constraint.
  ///
  /// [1]: https://w3.org/TR/mediacapture-streams#dom-constraindomstring
  void exactFacingMode(FacingMode facingMode) {
    _exactFacingMode(ptr.getInnerPtr(), facingMode.index);
  }

  /// Sets ideal [facingMode][1] constraint.
  ///
  /// [1]: https://w3.org/TR/mediacapture-streams#dom-constraindomstring
  void idealFacingMode(FacingMode facingMode) {
    _idealFacingMode(ptr.getInnerPtr(), facingMode.index);
  }

  /// Sets exact [`height`][1] constraint.
  ///
  /// [1]: https://tinyurl.com/w3-streams#def-constraint-height
  void exactHeight(int height) {
    _exactHeight(ptr.getInnerPtr(), height);
  }

  /// Sets ideal [`height`][1] constraint.
  ///
  /// [1]: https://tinyurl.com/w3-streams#def-constraint-height
  void idealHeight(int height) {
    _idealHeight(ptr.getInnerPtr(), height);
  }

  /// Sets range of [`height`][1] constraint.
  ///
  /// [1]: https://tinyurl.com/w3-streams#def-constraint-height
  void heightInRange(int min, int max) {
    _heightInRange(ptr.getInnerPtr(), min, max);
  }

  /// Sets exact [`width`][1] constraint.
  ///
  /// [1]: https://tinyurl.com/w3-streams#def-constraint-width
  void exactWidth(int width) {
    _exactWidth(ptr.getInnerPtr(), width);
  }

  /// Sets ideal [`width`][1] constraint.
  ///
  /// [1]: https://tinyurl.com/w3-streams#def-constraint-width
  void idealWidth(int width) {
    _idealWidth(ptr.getInnerPtr(), width);
  }

  /// Sets range of [`width`][1] constraint.
  ///
  /// [1]: https://tinyurl.com/w3-streams#def-constraint-width
  void widthInRange(int min, int max) {
    _widthInRange(ptr.getInnerPtr(), min, max);
  }

  /// Drops associated Rust object and nulls the local [Pointer] to this object.
  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
