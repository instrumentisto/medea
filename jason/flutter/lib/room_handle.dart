import 'dart:ffi';

import 'package:medea_jason/local_media_track.dart';
import 'package:medea_jason/util/nullable_pointer.dart';

import 'connection_handle.dart';
import 'jason.dart';
import 'reconnect_handle.dart';
import 'room_close_reason.dart';
import 'util/move_semantic.dart';

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

typedef _onNewConnection_C = Void Function(Pointer, Handle);
typedef _onNewConnection_Dart = void Function(Pointer, void Function(Pointer));

typedef _onClose_C = Void Function(Pointer, Handle);
typedef _onClose_Dart = void Function(Pointer, void Function(Pointer));

typedef _onLocalTrack_C = Void Function(Pointer, Handle);
typedef _onLocalTrack_Dart = void Function(Pointer, void Function(Pointer));

typedef _onConnectionLoss_C = Void Function(Pointer, Handle);
typedef _onConnectionLoss_Dart = void Function(Pointer, void Function(Pointer));

final _free = dl.lookupFunction<_free_C, _free_Dart>('RoomHandle__free');

final _onNewConnection =
    dl.lookupFunction<_onNewConnection_C, _onNewConnection_Dart>(
        'RoomHandle__on_new_connection');

final _onClose =
    dl.lookupFunction<_onClose_C, _onClose_Dart>('RoomHandle__on_close');

final _onLocalTrack = dl.lookupFunction<_onLocalTrack_C, _onLocalTrack_Dart>(
    'RoomHandle__on_local_track');

final _onConnectionLoss =
    dl.lookupFunction<_onConnectionLoss_C, _onConnectionLoss_Dart>(
        'RoomHandle__on_connection_loss');

class RoomHandle {
  /// [Pointer] to Rust struct that backs this object.
  late NullablePointer ptr;

  /// Constructs new [RoomHandle] backed by Rust object behind provided
  /// [Pointer].
  RoomHandle(this.ptr);

  /// Sets callback, invoked when a new `Connection` with some remote `Peer`
  /// is established.
  void onNewConnection(void Function(ConnectionHandle) f) {
    _onNewConnection(ptr.getInnerPtr(), (t) {
      f(ConnectionHandle(NullablePointer(t)));
    });
  }

  /// Sets callback, invoked on this `Room` close, providing a
  /// [RoomCloseReason].
  void onClose(void Function(RoomCloseReason) f) {
    _onClose(ptr.getInnerPtr(), (t) {
      f(RoomCloseReason(NullablePointer(t)));
    });
  }

  /// Sets callback, invoked when a new [LocalMediaTrack] is added to this
  /// `Room`.
  ///
  /// This might happen in such cases:
  /// 1. Media server initiates a media request.
  /// 2. [RoomHandle.enableAudio()]/[RoomHandle.enableVideo()] is called.
  /// 3. [MediaStreamSettings] were updated with
  /// [RoomHandle.setLocalMediaSettings()] call.
  void onLocalTrack(void Function(LocalMediaTrack) f) {
    _onLocalTrack(ptr.getInnerPtr(), (t) {
      f(LocalMediaTrack(NullablePointer(t)));
    });
  }

  /// Sets callback, invoked when a connection with server is lost, providing
  /// [ReconnectHandle].
  void onConnectionLoss(void Function(ReconnectHandle) f) {
    _onConnectionLoss(ptr.getInnerPtr(), (t) {
      f(ReconnectHandle(NullablePointer(t)));
    });
  }

  /// Drops associated Rust object and nulls the local [Pointer] to this object.
  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
