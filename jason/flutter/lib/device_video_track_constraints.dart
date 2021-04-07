import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'ffi.dart' as ffi;

final _deviceIdDart _deviceId = ffi.dl
    .lookupFunction<_deviceIdC, _deviceIdDart>(
        'DeviceVideoTrackConstraints__device_id');
typedef _deviceIdC = Pointer<Utf8> Function(Pointer);
typedef _deviceIdDart = Pointer<Utf8> Function(Pointer);

final _exactFacingModeDart _exactFacingMode = ffi.dl
    .lookupFunction<_exactFacingModeC, _exactFacingModeDart>(
        'DeviceVideoTrackConstraints__exact_facing_mode');
typedef _exactFacingModeC = Void Function(Pointer, int);
typedef _exactFacingModeDart = void Function(Pointer, int);

final _idealFacingModeDart _idealFacingMode = ffi.dl
    .lookupFunction<_idealFacingModeC, _idealFacingModeDart>(
        'DeviceVideoTrackConstraints__ideal_facing_mode');
typedef _idealFacingModeC = Void Function(Pointer, int);
typedef _idealFacingModeDart = void Function(Pointer, int);

final _exactHeightDart _exactHeight = ffi.dl
    .lookupFunction<_exactHeightC, _exactHeightDart>(
        'DeviceVideoTrackConstraints__exact_height');
typedef _exactHeightC = Void Function(Pointer, int);
typedef _exactHeightDart = void Function(Pointer, int);

final _idealHeightDart _idealHeight = ffi.dl
    .lookupFunction<_idealHeightC, _idealHeightDart>(
        'DeviceVideoTrackConstraints__ideal_height');
typedef _idealHeightC = Void Function(Pointer, int);
typedef _idealHeightDart = void Function(Pointer, int);

final _heightInRangeDart _heightInRange = ffi.dl
    .lookupFunction<_heightInRangeC, _heightInRangeDart>(
        'DeviceVideoTrackConstraints__height_in_range');
typedef _heightInRangeC = Void Function(Pointer, int, int);
typedef _heightInRangeDart = void Function(Pointer, int, int);

final _exactWidthDart _exactWidth = ffi.dl
    .lookupFunction<_exactWidthC, _exactWidthDart>(
        'DeviceVideoTrackConstraints__exact_width');
typedef _exactWidthC = Void Function(Pointer, int);
typedef _exactWidthDart = void Function(Pointer, int);

final _idealWidthDart _idealWidth = ffi.dl
    .lookupFunction<_idealWidthC, _idealWidthDart>(
        'DeviceVideoTrackConstraints__ideal_width');
typedef _idealWidthC = Void Function(Pointer, int);
typedef _idealWidthDart = void Function(Pointer, int);

final _widthInRangeDart _widthInRange = ffi.dl
    .lookupFunction<_widthInRangeC, _widthInRangeDart>(
        'DeviceVideoTrackConstraints__width_in_range');
typedef _widthInRangeC = Void Function(Pointer, int, int);
typedef _widthInRangeDart = void Function(Pointer, int, int);

enum FacingMode {
  User,
  Environment,
  Left,
  Right,
}

int facingModeToInt(FacingMode facingMode) {
  switch (facingMode) {
    case FacingMode.User:
      return 0;
    case FacingMode.Environment:
      return 1;
    case FacingMode.Left:
      return 2;
    case FacingMode.Right:
      return 3;
  }
  throw Exception("Unknown enum variant");
}

class DeviceVideoTrackConstraints {
  late Pointer ptr;

  DeviceVideoTrackConstraints(Pointer p) {
    ptr = p;
  }

  String deviceId() {
    return _deviceId(ptr).toDartString();
  }

  void exactFacingMode(FacingMode facingMode) {
    _exactFacingMode(ptr, facingModeToInt(facingMode));
  }

  void idealFacingMode(FacingMode facingMode) {
    _idealFacingMode(ptr, facingModeToInt(facingMode));
  }

  void exactHeight(int height) {
    _exactHeight(ptr, height);
  }

  void idealHeight(int height) {
    _idealHeight(ptr, height);
  }

  void heightInRange(int min, int max) {
    _heightInRange(ptr, min, max);
  }

  void exactWidth(int width) {
    _exactWidth(ptr, width);
  }

  void idealWidth(int width) {
    _idealWidth(ptr, width);
  }

  void widthInRange(int min, int max) {
    _widthInRange(ptr, min, max);
  }
}
