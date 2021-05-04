import 'dart:ffi';

import 'package:medea_jason/media_stream_settings.dart';

import 'connection_handle.dart';
import 'jason.dart';
import 'local_media_track.dart';
import 'reconnect_handle.dart';
import 'room_close_reason.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';

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

/// External handle to a `Room`.
class RoomHandle {
  /// [Pointer] to the Rust struct that backing this object.
  late NullablePointer ptr;

  /// Constructs a new [RoomHandle] backed by the Rust struct behind the
  /// provided [Pointer].
  RoomHandle(this.ptr);

  /// Sets callback, invoked when a new `Connection` with some remote `Peer`
  /// is established.
  void onNewConnection(void Function(ConnectionHandle) f) {
    _onNewConnection(ptr.getInnerPtr(), (t) {
      f(ConnectionHandle(NullablePointer(t)));
    });
  }

  /// Sets callback, invoked when this `Room` is closed, providing a
  /// [RoomCloseReason].
  void onClose(void Function(RoomCloseReason) f) {
    _onClose(ptr.getInnerPtr(), (t) {
      f(RoomCloseReason(NullablePointer(t)));
    });
  }

  Future<void> setLocalMediaSettings(
    MediaStreamSettings constraints,
    bool stopFirst,
    bool rollbackOnFail,
  ) async {
    throw UnimplementedError();
  }

  Future<void> disableAudio() async {
    throw UnimplementedError();
  }

  Future<void> enableAudio() async {
    throw UnimplementedError();
  }

  Future<void> disableVideo() async {
    throw UnimplementedError();
  }

  Future<void> enableVideo() async {
    throw UnimplementedError();
  }

  /// Sets callback, invoked when a new [LocalMediaTrack] is added to this
  /// `Room`.
  ///
  /// This might happen in the following cases:
  /// 1. Media server initiates a media request.
  /// 2. [RoomHandle.enableAudio()]/[RoomHandle.enableVideo()] is called.
  /// 3. [MediaStreamSettings] were updated via
  ///    [RoomHandle.setLocalMediaSettings()] method.
  void onLocalTrack(void Function(LocalMediaTrack) f) {
    _onLocalTrack(ptr.getInnerPtr(), (t) {
      f(LocalMediaTrack(NullablePointer(t)));
    });
  }

  /// Sets callback, invoked when a connection with a media server is lost,
  /// providing a [ReconnectHandle].
  void onConnectionLoss(void Function(ReconnectHandle) f) {
    _onConnectionLoss(ptr.getInnerPtr(), (t) {
      f(ReconnectHandle(NullablePointer(t)));
    });
  }

  /// Drops the associated Rust struct and nulls the local [Pointer] to it.
  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
