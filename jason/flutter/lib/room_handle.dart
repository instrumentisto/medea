import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'connection_handle.dart';
import 'jason.dart';
import 'track_kinds.dart';
import 'media_stream_settings.dart';
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

typedef _join_C = Handle Function(Pointer, Pointer<Utf8>);
typedef _join_Dart = Object Function(Pointer, Pointer<Utf8>);

typedef _setLocalMediaSettings_C = Handle Function(
    Pointer, Pointer, Uint8, Uint8);
typedef _setLocalMediaSettings_Dart = Object Function(
    Pointer, Pointer, int, int);

typedef _muteAudio_C = Handle Function(Pointer);
typedef _muteAudio_Dart = Object Function(Pointer);

typedef _unmuteAudio_C = Handle Function(Pointer);
typedef _unmuteAudio_Dart = Object Function(Pointer);

typedef _muteVideo_C = Handle Function(Pointer, Uint8);
typedef _muteVideo_Dart = Object Function(Pointer, int);

typedef _unmuteVideo_C = Handle Function(Pointer, Uint8);
typedef _unmuteVideo_Dart = Object Function(Pointer, int);

typedef _disableVideo_C = Handle Function(Pointer, Uint8);
typedef _disableVideo_Dart = Object Function(Pointer, int);

typedef _enableVideo_C = Handle Function(Pointer, Uint8);
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

final _join = dl.lookupFunction<_join_C, _join_Dart>('RoomHandle__join');

final _setLocalMediaSettings =
    dl.lookupFunction<_setLocalMediaSettings_C, _setLocalMediaSettings_Dart>(
        'RoomHandle__set_local_media_settings');

final _muteAudio =
    dl.lookupFunction<_muteAudio_C, _muteAudio_Dart>('RoomHandle__mute_audio');

final _unmuteAudio = dl.lookupFunction<_unmuteAudio_C, _unmuteAudio_Dart>(
    'RoomHandle__unmute_audio');

final _muteVideo =
    dl.lookupFunction<_muteVideo_C, _muteVideo_Dart>('RoomHandle__mute_video');

final _unmuteVideo = dl.lookupFunction<_unmuteVideo_C, _unmuteVideo_Dart>(
    'RoomHandle__unmute_video');

final _disableVideo = dl.lookupFunction<_disableVideo_C, _disableVideo_Dart>(
    'RoomHandle__disable_video');

final _enableVideo = dl.lookupFunction<_enableVideo_C, _enableVideo_Dart>(
    'RoomHandle__enable_video');

final _disableAudio = dl.lookupFunction<_disableAudio_C, _disableAudio_Dart>(
    'RoomHandle__disable_audio');

final _enableAudio = dl.lookupFunction<_enableAudio_C, _enableAudio_Dart>(
    'RoomHandle__enable_audio');

final _disableRemoteAudio =
    dl.lookupFunction<_disableRemoteAudio_C, _disableRemoteAudio_Dart>(
        'RoomHandle__disable_remote_audio');

final _enableRemoteAudio =
    dl.lookupFunction<_enableRemoteAudio_C, _enableRemoteAudio_Dart>(
        'RoomHandle__enable_remote_audio');

final _disableRemoteVideo =
    dl.lookupFunction<_disableRemoteVideo_C, _disableRemoteVideo_Dart>(
        'RoomHandle__disable_remote_video');

final _enableRemoteVideo =
    dl.lookupFunction<_enableRemoteVideo_C, _enableRemoteVideo_Dart>(
        'RoomHandle__enable_remote_video');

/// External handle to a `Room`.
class RoomHandle {
  /// [Pointer] to the Rust struct that backing this object.
  late NullablePointer ptr;

  /// Constructs a new [RoomHandle] backed by the Rust struct behind the
  /// provided [Pointer].
  RoomHandle(this.ptr);

  /// Connects to a media server and joins the `Room` with the provided
  /// authorization [token].
  ///
  /// Authorization token has a fixed format:
  /// `{{ Host URL }}/{{ Room ID }}/{{ Member ID }}?token={{ Auth Token }}`
  /// (e.g. `wss://medea.com/MyConf1/Alice?token=777`).
  Future<void> join(String token) async {
    var tokenPtr = token.toNativeUtf8();
    try {
      await (_join(ptr.getInnerPtr(), tokenPtr) as Future);
    } finally {
      calloc.free(tokenPtr);
    }
  }

  /// Updates this `Room`'s [MediaStreamSettings]. This affects all the
  /// `PeerConnection`s in this `Room`. If [MediaStreamSettings] are configured
  /// for some `Room`, then this `Room` can only send media tracks that
  /// correspond to these settings. [MediaStreamSettings] update will change
  /// media tracks in all sending peers, so that might cause a new
  /// [getUserMedia()][1] request to happen.
  ///
  /// Media obtaining/injection errors are additionally fired to
  /// [RoomHandle.onFailedLocalMedia()] callback.
  ///
  /// If [stop_first] set to `true` then affected local [LocalMediaTrack]s will
  /// be dropped before new [MediaStreamSettings] are applied. This is usually
  /// required when changing video source device due to hardware limitations,
  /// e.g. having an active track sourced from device `A` may hinder
  /// [getUserMedia()][1] requests to device `B`.
  ///
  /// [rollback_on_fail] option configures [MediaStreamSettings] update request
  /// to automatically rollback to previous settings if new settings cannot be
  /// applied.
  ///
  /// If recovering from fail state isn't possible then affected media types
  /// will be disabled.
  ///
  /// [1]: https://w3.org/TR/mediacapture-streams#dom-mediadevices-getusermedia
  Future<void> setLocalMediaSettings(
      MediaStreamSettings settings, bool stopFirst, bool rollbackOnFail) async {
    await (_setLocalMediaSettings(ptr.getInnerPtr(), settings.ptr.getInnerPtr(),
        stopFirst ? 1 : 0, rollbackOnFail ? 1 : 0) as Future);
  }

  /// Mutes outbound audio in this `Room`.
  Future<void> muteAudio() async {
    await (_muteAudio(ptr.getInnerPtr()) as Future);
  }

  /// Unmutes outbound audio in this `Room`.
  Future<void> unmuteAudio() async {
    await (_unmuteAudio(ptr.getInnerPtr()) as Future);
  }

  /// Enables outbound audio in this `Room`.
  Future<void> enableAudio() async {
    await (_enableAudio(ptr.getInnerPtr()) as Future);
  }

  /// Disables outbound audio in this `Room`.
  Future<void> disableAudio() async {
    await (_disableAudio(ptr.getInnerPtr()) as Future);
  }

  /// Mutes outbound video in this `Room`.
  ///
  /// Affects only video with specific [MediaSourceKind] if specified.
  Future<void> muteVideo(MediaSourceKind kind) async {
    await (_muteVideo(ptr.getInnerPtr(), kind.index) as Future);
  }

  /// Unmutes outbound video in this `Room`.
  ///
  /// Affects only video with specific [MediaSourceKind] if specified.
  Future<void> unmuteVideo(MediaSourceKind kind) async {
    await (_unmuteVideo(ptr.getInnerPtr(), kind.index) as Future);
  }

  /// Enables outbound video.
  ///
  /// Affects only video with specific [MediaSourceKind] if specified.
  Future<void> enableVideo(MediaSourceKind kind) async {
    await (_enableVideo(ptr.getInnerPtr(), kind.index) as Future);
  }

  /// Disables outbound video.
  ///
  /// Affects only video with specific [MediaSourceKind] if specified.
  Future<void> disableVideo(MediaSourceKind kind) async {
    await (_disableVideo(ptr.getInnerPtr(), kind.index) as Future);
  }

  /// Enables inbound audio in this `Room`.
  Future<void> enableRemoteAudio() async {
    await (_enableRemoteAudio(ptr.getInnerPtr()) as Future);
  }

  /// Disables inbound audio in this `Room`.
  Future<void> disableRemoteAudio() async {
    await (_disableRemoteAudio(ptr.getInnerPtr()) as Future);
  }

  /// Enables inbound video in this `Room`.
  Future<void> enableRemoteVideo() async {
    await (_enableRemoteVideo(ptr.getInnerPtr()) as Future);
  }

  /// Disables inbound video in this `Room`.
  Future<void> disableRemoteVideo() async {
    await (_disableRemoteVideo(ptr.getInnerPtr()) as Future);
  }

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
