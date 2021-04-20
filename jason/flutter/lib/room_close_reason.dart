import 'dart:ffi';
import 'package:ffi/ffi.dart';

import 'jason.dart';
import 'util/move_semantic.dart';
import 'util/native_string.dart';
import 'util/nullable_pointer.dart';

typedef _reason_C = Pointer<Utf8> Function(Pointer);
typedef _reason_Dart = Pointer<Utf8> Function(Pointer);

typedef _isClosedByServer_C = Int8 Function(Pointer);
typedef _isClosedByServer_Dart = int Function(Pointer);

typedef _isErr_C = Int8 Function(Pointer);
typedef _isErr_Dart = int Function(Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _reason =
    dl.lookupFunction<_reason_C, _reason_Dart>('RoomCloseReason__reason');

final _isClosedByServer =
    dl.lookupFunction<_isClosedByServer_C, _isClosedByServer_Dart>(
        'RoomCloseReason__is_closed_by_server');

final _isErr =
    dl.lookupFunction<_isErr_C, _isErr_Dart>('RoomCloseReason__is_err');

final _free = dl.lookupFunction<_free_C, _free_Dart>('RoomCloseReason__free');

class RoomCloseReason {
  late NullablePointer ptr;

  RoomCloseReason(this.ptr);

  String reason() {
    return _reason(ptr.getInnerPtr()).nativeStringToDartString();
  }

  bool isClosedByServer() {
    return _isClosedByServer(ptr.getInnerPtr()) > 0;
  }

  bool isErr() {
    return _isErr(ptr.getInnerPtr()) > 0;
  }

  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
