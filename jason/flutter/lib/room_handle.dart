import 'dart:ffi';
import 'package:ffi/ffi.dart';

import 'package:medea_jason/local_media_track.dart';
import 'package:medea_jason/util/nullable_pointer.dart';

import 'connection_handle.dart';
import 'jason.dart';
import 'kind.dart';
import 'media_stream_settings.dart';
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

typedef _join_C = Handle Function(Pointer, Pointer<Utf8>);
typedef _join_Dart = Object Function(Pointer, Pointer<Utf8>);

typedef _setLocalMediaSettings_C = Handle Function(
    Pointer, Pointer, Int64, Int64);
typedef _setLocalMediaSettings_Dart = Object Function(
    Pointer, Pointer, int, int);

typedef _muteAudio_C = Handle Function(Pointer);
typedef _muteAudio_Dart = Object Function(Pointer);

typedef _unmuteAudio_C = Handle Function(Pointer);
typedef _unmuteAudio_Dart = Object Function(Pointer);

typedef _muteVideo_C = Handle Function(Pointer, Int64);
typedef _muteVideo_Dart = Object Function(Pointer, int);

typedef _unmuteVideo_C = Handle Function(Pointer, Int64);
typedef _unmuteVideo_Dart = Object Function(Pointer, int);

typedef _disableVideo_C = Handle Function(Pointer, Int64);
typedef _disableVideo_Dart = Object Function(Pointer, int);

typedef _enableVideo_C = Handle Function(Pointer, Int64);
typedef _enableVideo_Dart = Object Function(Pointer, int);

typedef _disableAudio_C = Handle Function(Pointer);
typedef _disableAudio_Dart = Object Function(Pointer);

typedef _enableAudio_C = Handle Function(Pointer);
typedef _enableAudio_Dart = Object Function(Pointer);

typedef _disableRemoteAudio_C = Handle Function(Pointer);
typedef _disableRemoteAudio_Dart = Object Function(Pointer);

typedef _enableRemoteAudio_C = Handle Function(Pointer);
typedef _enableRemoteAudio_Dart = Object Function(Pointer);

typedef _disableRemoteVideo_C = Handle Function(Pointer);
typedef _disableRemoteVideo_Dart = Object Function(Pointer);

typedef _enableRemoteVideo_C = Handle Function(Pointer);
typedef _enableRemoteVideo_Dart = Object Function(Pointer);

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

final _join = dl.lookupFunction<_join_C, _join_Dart>('ConnectionHandle__join');

final _setLocalMediaSettings =
    dl.lookupFunction<_setLocalMediaSettings_C, _setLocalMediaSettings_Dart>(
        'ConnectionHandle__set_local_media_settings');

final _muteAudio = dl.lookupFunction<_muteAudio_C, _muteAudio_Dart>(
    'ConnectionHandle__mute_audio');

final _unmuteAudio = dl.lookupFunction<_unmuteAudio_C, _unmuteAudio_Dart>(
    'ConnectionHandle__unmute_audio');

final _muteVideo = dl.lookupFunction<_muteVideo_C, _muteVideo_Dart>(
    'ConnectionHandle__mute_video');

final _unmuteVideo = dl.lookupFunction<_unmuteVideo_C, _unmuteVideo_Dart>(
    'ConnectionHandle__unmute_video');

final _disableVideo = dl.lookupFunction<_disableVideo_C, _disableVideo_Dart>(
    'ConnectionHandle__disable_video');

final _enableVideo = dl.lookupFunction<_enableVideo_C, _enableVideo_Dart>(
    'ConnectionHandle__enable_video');

final _disableAudio = dl.lookupFunction<_disableAudio_C, _disableAudio_Dart>(
    'ConnectionHandle__disable_audio');

final _enableAudio = dl.lookupFunction<_enableAudio_C, _enableAudio_Dart>(
    'ConnectionHandle__enable_audio');

final _disableRemoteAudio =
    dl.lookupFunction<_disableRemoteAudio_C, _disableRemoteAudio_Dart>(
        'ConnectionHandle__disable_remote_audio');

final _enableRemoteAudio =
    dl.lookupFunction<_enableRemoteAudio_C, _enableRemoteAudio_Dart>(
        'ConnectionHandle__enable_remote_audio');

final _disableRemoteVideo =
    dl.lookupFunction<_disableRemoteVideo_C, _disableRemoteVideo_Dart>(
        'ConnectionHandle__disable_remote_video');

final _enableRemoteVideo =
    dl.lookupFunction<_enableRemoteVideo_C, _enableRemoteVideo_Dart>(
        'ConnectionHandle__enable_remote_video');

class RoomHandle {
  late NullablePointer ptr;

  RoomHandle(this.ptr);

  Future<void> join(String url) async {
    var fut = _join(ptr.getInnerPtr(), url.toNativeUtf8());
    if (fut is Future) {
      await fut;
    } else {
      throw Exception(
          'Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  Future<void> setLocalMediaSettings(
      MediaStreamSettings settings, bool stopFirst, bool rollbackOnFail) async {
    var fut = _setLocalMediaSettings(ptr.getInnerPtr(),
        settings.ptr.getInnerPtr(), stopFirst ? 1 : 0, rollbackOnFail ? 1 : 0);
    if (fut is Future) {
      await fut;
    }
    {
      throw Exception(
          'Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  Future<void> muteAudio() async {
    var fut = _muteAudio(ptr.getInnerPtr());
    if (fut is Future) {
      await fut;
    }
    {
      throw Exception(
          'Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  Future<void> unmuteAudio() async {
    var fut = _unmuteAudio(ptr.getInnerPtr());
    if (fut is Future) {
      await fut;
    }
    {
      throw Exception(
          'Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  Future<void> muteVideo(MediaSourceKind kind) async {
    var fut = _muteVideo(ptr.getInnerPtr(), nativeMediaSourceKind(kind));
    if (fut is Future) {
      await fut;
    }
    {
      throw Exception(
          'Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  Future<void> unmuteVideo(MediaSourceKind kind) async {
    var fut = _unmuteVideo(ptr.getInnerPtr(), nativeMediaSourceKind(kind));
    if (fut is Future) {
      await fut;
    }
    {
      throw Exception(
          'Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  Future<void> disableVideo(MediaSourceKind kind) async {
    var fut = _disableVideo(ptr.getInnerPtr(), nativeMediaSourceKind(kind));
    if (fut is Future) {
      await fut;
    }
    {
      throw Exception(
          'Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  Future<void> enableVideo(MediaSourceKind kind) async {
    var fut = _enableVideo(ptr.getInnerPtr(), nativeMediaSourceKind(kind));
    if (fut is Future) {
      await fut;
    }
    {
      throw Exception(
          'Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  Future<void> disableAudio() async {
    var fut = _disableAudio(ptr.getInnerPtr());
    if (fut is Future) {
      await fut;
    }
    {
      throw Exception(
          'Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  Future<void> enableAudio() async {
    var fut = _enableAudio(ptr.getInnerPtr());
    if (fut is Future) {
      await fut;
    }
    {
      throw Exception(
          'Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  Future<void> disableRemoteAudio() async {
    var fut = _disableRemoteAudio(ptr.getInnerPtr());
    if (fut is Future) {
      await fut;
    }
    {
      throw Exception(
          'Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  Future<void> enableRemoteAudio() async {
    var fut = _enableRemoteAudio(ptr.getInnerPtr());
    if (fut is Future) {
      await fut;
    }
    {
      throw Exception(
          'Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  Future<void> disableRemoteVideo() async {
    var fut = _disableRemoteVideo(ptr.getInnerPtr());
    if (fut is Future) {
      await fut;
    }
    {
      throw Exception(
          'Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  Future<void> enableRemoteVideo() async {
    var fut = _enableRemoteVideo(ptr.getInnerPtr());
    if (fut is Future) {
      await fut;
    }
    {
      throw Exception(
          'Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

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

  void onLocalTrack(void Function(LocalMediaTrack) f) {
    _onLocalTrack(ptr.getInnerPtr(), (t) {
      f(LocalMediaTrack(NullablePointer(t)));
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
