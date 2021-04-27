import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'jason.dart';
import 'remote_media_track.dart';
import 'util/move_semantic.dart';
import 'util/native_string.dart';
import 'util/nullable_pointer.dart';

typedef _getRemoteMemberId_C = Pointer<Utf8> Function(Pointer);
typedef _getRemoteMemberId_Dart = Pointer<Utf8> Function(Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

typedef _onClose_C = Void Function(Pointer, Handle);
typedef _onClose_Dart = void Function(Pointer, void Function());

typedef _onRemoteTrackAdded_C = Void Function(Pointer, Handle);
typedef _onRemoteTrackAdded_Dart = void Function(
    Pointer, void Function(Pointer));

typedef _onQualityScoreUpdate_C = Void Function(Pointer, Handle);
typedef _onQualityScoreUpdate_Dart = void Function(Pointer, void Function(int));

final _getRemoteMemberId =
    dl.lookupFunction<_getRemoteMemberId_C, _getRemoteMemberId_Dart>(
        'ConnectionHandle__get_remote_member_id');

final _free = dl.lookupFunction<_free_C, _free_Dart>('ConnectionHandle__free');

final _onClose =
    dl.lookupFunction<_onClose_C, _onClose_Dart>('ConnectionHandle__on_close');

final _onRemoteTrackAdded =
    dl.lookupFunction<_onRemoteTrackAdded_C, _onRemoteTrackAdded_Dart>(
        'ConnectionHandle__on_remote_track_added');

final _onQualityScoreUpdate =
    dl.lookupFunction<_onQualityScoreUpdate_C, _onQualityScoreUpdate_Dart>(
        'ConnectionHandle__on_quality_score_update');

/// External handler to a `Connection` with a remote `Member`.
class ConnectionHandle {
  /// [Pointer] to Rust struct that backs this object.
  late NullablePointer ptr;

  /// Constructs new [ConnectionHandle] backed by Rust object behind provided
  /// [Pointer].
  ConnectionHandle(this.ptr);

  /// Returns remote `Member` ID.
  String getRemoteMemberId() {
    return _getRemoteMemberId(ptr.getInnerPtr()).nativeStringToDartString();
  }

  /// Sets callback, invoked when this `Connection` will close.
  void onClose(void Function() f) {
    _onClose(ptr.getInnerPtr(), f);
  }

  /// Sets callback, invoked when a new [RemoteMediaTrack] is added to this
  /// `Connection`.
  void onRemoteTrackAdded(void Function(RemoteMediaTrack) f) {
    _onRemoteTrackAdded(ptr.getInnerPtr(), (t) {
      f(RemoteMediaTrack(NullablePointer(t)));
    });
  }

  /// Sets callback, invoked when a connection quality score is updated by a
  /// server.
  void onQualityScoreUpdate(void Function(int) f) {
    _onQualityScoreUpdate(ptr.getInnerPtr(), f);
  }

  /// Drops associated Rust object and nulls the local [Pointer] to this object.
  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
