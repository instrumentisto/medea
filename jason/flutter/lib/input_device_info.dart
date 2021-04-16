import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'jason.dart';
import 'kind.dart';
import 'util/errors.dart';
import 'util/move_semantic.dart';

typedef _deviceId_C = Pointer<Utf8> Function(Pointer);
typedef _deviceId_Dart = Pointer<Utf8> Function(Pointer);

typedef _label_C = Pointer<Utf8> Function(Pointer);
typedef _label_Dart = Pointer<Utf8> Function(Pointer);

typedef _kind_C = Int16 Function(Pointer);
typedef _kind_Dart = int Function(Pointer);

typedef _nativeGroupId_C = Pointer<Utf8> Function(Pointer);
typedef _nativeGroupId_Dart = Pointer<Utf8> Function(Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _nativeGroupId_Dart _nativeGroupId =
    dl.lookupFunction<_nativeGroupId_C, _nativeGroupId_Dart>(
        'InputDeviceInfo__group_id');
final _kind_Dart _kind =
    dl.lookupFunction<_kind_C, _kind_Dart>('InputDeviceInfo__kind');
final _label_Dart _label =
    dl.lookupFunction<_label_C, _label_Dart>('InputDeviceInfo__label');
final _deviceId_Dart _deviceId = dl
    .lookupFunction<_deviceId_C, _deviceId_Dart>('InputDeviceInfo__device_id');

final _free_Dart _free =
    dl.lookupFunction<_free_C, _free_Dart>('InputDeviceInfo__free');

class InputDeviceInfo {
  late Pointer ptr;

  InputDeviceInfo(Pointer p) {
    assertNonNull(p);

    ptr = p;
  }

  String deviceId() {
    assertNonNull(ptr);

    return _deviceId(ptr).toDartString();
  }

  String label() {
    assertNonNull(ptr);

    return _label(ptr).toDartString();
  }

  MediaKind kind() {
    assertNonNull(ptr);

    var index = _kind(ptr);
    return MediaKind.values[index];
  }

  String groupId() {
    assertNonNull(ptr);

    return _nativeGroupId(ptr).toDartString();
  }

  @moveSemantics
  void free() {
    _free(ptr);
  }
}
