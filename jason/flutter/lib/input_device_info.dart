import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'jason.dart';
import 'kind.dart';
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

class InputDeviceInfo {
  late NullablePointer ptr;

  InputDeviceInfo(this.ptr);

  String deviceId() {
    return _deviceId(ptr.getInnerPtr()).nativeStringToDartString();
  }

  String label() {
    return _label(ptr.getInnerPtr()).nativeStringToDartString();
  }

  MediaKind kind() {
    var index = _kind(ptr.getInnerPtr());
    return MediaKind.values[index];
  }

  String groupId() {
    return _nativeGroupId(ptr.getInnerPtr()).nativeStringToDartString();
  }

  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
