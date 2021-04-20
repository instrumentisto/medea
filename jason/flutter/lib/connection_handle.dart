import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'jason.dart';
import 'util/move_semantic.dart';
import 'util/native_string.dart';
import 'util/nullable_pointer.dart';

typedef _getRemoteMemberId_C = Pointer<Utf8> Function(Pointer);
typedef _getRemoteMemberId_Dart = Pointer<Utf8> Function(Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _getRemoteMemberId =
    dl.lookupFunction<_getRemoteMemberId_C, _getRemoteMemberId_Dart>(
        'ConnectionHandle__get_remote_member_id');

final _free = dl.lookupFunction<_free_C, _free_Dart>('ConnectionHandle__free');

class ConnectionHandle {
  late NullablePointer ptr;

  ConnectionHandle(this.ptr);

  String getRemoteMemberId() {
    return _getRemoteMemberId(ptr.getInnerPtr()).nativeStringToDartString();
  }

  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
