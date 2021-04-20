import 'dart:ffi';

import 'package:medea_jason/util/nullable_pointer.dart';

import 'connection_handle.dart';
import 'jason.dart';
import 'reconnect_handle.dart';
import 'remote_media_track.dart';
import 'room_close_reason.dart';
import 'util/move_semantic.dart';

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

typedef _onNewConnection_C = Void Function(Handle, Handle);
typedef _onNewConnection_Dart = void Function(Pointer, void Function(Pointer));

typedef _onClose_C = Void Function(Handle, Handle);
typedef _onClose_Dart = void Function(Pointer, void Function(Pointer));

typedef _onLocalTrack_C = Void Function(Handle, Handle);
typedef _onLocalTrack_Dart = void Function(Pointer, void Function(Pointer));

typedef _onConnectionLoss_C = Void Function(Handle, Handle);
typedef _onConnectionLoss_Dart = void Function(Pointer, void Function(Pointer));

final _free = dl.lookupFunction<_free_C, _free_Dart>('RoomHandle__free');

final _onNewConnection =
    dl.lookupFunction<_onNewConnection_C, _onNewConnection_Dart>(
        'ConnectionHandle__on_new_connection');

final _onClose =
    dl.lookupFunction<_onClose_C, _onClose_Dart>('ConnectionHandle__on_close');

final _onLocalTrack = dl.lookupFunction<_onLocalTrack_C, _onLocalTrack_Dart>(
    'ConnectionHandle__on_local_track');

final _onConnectionLoss =
    dl.lookupFunction<_onConnectionLoss_C, _onConnectionLoss_Dart>(
        'ConnectionHandle__on_connection_loss');

class RoomHandle {
  late NullablePointer ptr;

  RoomHandle(this.ptr);

  void onNewConnection(void Function(ConnectionHandle) f) {
    _onNewConnection(ptr.getInnerPtr(), (t) {
      f(ConnectionHandle(NullablePointer(t)));
    });
  }

  void onClose(void Function(RoomCloseReason) f) {
    _onClose(ptr.getInnerPtr(), (t) {
      f(RoomCloseReason(NullablePointer(t)));
    });
  }

  void onLocalTrack(void Function(RemoteMediaTrack) f) {
    _onLocalTrack(ptr.getInnerPtr(), (t) {
      f(RemoteMediaTrack(NullablePointer(t)));
    });
  }

  void onConnectionLoss(void Function(ReconnectHandle) f) {
    _onConnectionLoss(ptr.getInnerPtr(), (t) {
      f(ReconnectHandle(NullablePointer(t)));
    });
  }

  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
