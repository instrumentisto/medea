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

final _new_Dart _new =
    dl.lookupFunction<_new_C, _new_Dart>('DeviceVideoTrackConstraints__new');

final _deviceId_Dart _deviceId = dl.lookupFunction<_deviceId_C, _deviceId_Dart>(
    'DeviceVideoTrackConstraints__device_id');

final _exactFacingMode_Dart _exactFacingMode =
    dl.lookupFunction<_exactFacingMode_C, _exactFacingMode_Dart>(
        'DeviceVideoTrackConstraints__exact_facing_mode');

final _idealFacingMode_Dart _idealFacingMode =
    dl.lookupFunction<_idealFacingMode_C, _idealFacingMode_Dart>(
        'DeviceVideoTrackConstraints__ideal_facing_mode');

final _exactHeight_Dart _exactHeight =
    dl.lookupFunction<_exactHeight_C, _exactHeight_Dart>(
        'DeviceVideoTrackConstraints__exact_height');

final _idealHeight_Dart _idealHeight =
    dl.lookupFunction<_idealHeight_C, _idealHeight_Dart>(
        'DeviceVideoTrackConstraints__ideal_height');

final _heightInRange_Dart _heightInRange =
    dl.lookupFunction<_heightInRange_C, _heightInRange_Dart>(
        'DeviceVideoTrackConstraints__height_in_range');

final _exactWidth_Dart _exactWidth =
    dl.lookupFunction<_exactWidth_C, _exactWidth_Dart>(
        'DeviceVideoTrackConstraints__exact_width');

final _idealWidth_Dart _idealWidth =
    dl.lookupFunction<_idealWidth_C, _idealWidth_Dart>(
        'DeviceVideoTrackConstraints__ideal_width');

final _widthInRange_Dart _widthInRange =
    dl.lookupFunction<_widthInRange_C, _widthInRange_Dart>(
        'DeviceVideoTrackConstraints__width_in_range');

final _free_Dart _free =
    dl.lookupFunction<_free_C, _free_Dart>('DeviceVideoTrackConstraints__free');

enum FacingMode {
  User,
  Environment,
  Left,
  Right,
}

class DeviceVideoTrackConstraints {
  final NullablePointer ptr = NullablePointer(_new());

  void deviceId(String deviceId) {
    var deviceIdPtr = deviceId.toNativeUtf8();
    try {
      _deviceId(ptr.getInnerPtr(), deviceIdPtr);
    } finally {
      calloc.free(deviceIdPtr);
    }
  }

  void exactFacingMode(FacingMode facingMode) {
    _exactFacingMode(ptr.getInnerPtr(), facingMode.index);
  }

  void idealFacingMode(FacingMode facingMode) {
    _idealFacingMode(ptr.getInnerPtr(), facingMode.index);
  }

  void exactHeight(int height) {
    _exactHeight(ptr.getInnerPtr(), height);
  }

  void idealHeight(int height) {
    _idealHeight(ptr.getInnerPtr(), height);
  }

  void heightInRange(int min, int max) {
    _heightInRange(ptr.getInnerPtr(), min, max);
  }

  void exactWidth(int width) {
    _exactWidth(ptr.getInnerPtr(), width);
  }

  void idealWidth(int width) {
    _idealWidth(ptr.getInnerPtr(), width);
  }

  void widthInRange(int min, int max) {
    _widthInRange(ptr.getInnerPtr(), min, max);
  }

  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
