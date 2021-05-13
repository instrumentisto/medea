import 'dart:ffi';

import 'jason.dart';
import 'remote_media_track.dart';
import 'util/move_semantic.dart';
import 'ffi/native_string.dart';
import 'util/nullable_pointer.dart';
import 'ffi/result.dart';

typedef _getRemoteMemberId_C = Result Function(Pointer);
typedef _getRemoteMemberId_Dart = Result Function(Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

typedef _onClose_C = Result Function(Pointer, Handle);
typedef _onClose_Dart = Result Function(Pointer, void Function());

typedef _onRemoteTrackAdded_C = Result Function(Pointer, Handle);
typedef _onRemoteTrackAdded_Dart = Result Function(
    Pointer, void Function(Pointer));

typedef _onQualityScoreUpdate_C = Result Function(Pointer, Handle);
typedef _onQualityScoreUpdate_Dart = Result Function(
    Pointer, void Function(int));

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
  /// [Pointer] to the Rust struct backing this object.
  late NullablePointer ptr;

  /// Constructs a new [ConnectionHandle] backed by a Rust struct behind the
  /// provided [Pointer].
  ConnectionHandle(this.ptr);

  /// Returns ID of the remote `Member`.
  ///
  /// Throws [RustException] if Rust returns error.
  String getRemoteMemberId() {
    return _getRemoteMemberId(ptr.getInnerPtr()).unwrap();
  }

  /// Sets callback, invoked when this `Connection` is closed.
  ///
  /// Throws [RustException] if Rust returns error.
  void onClose(void Function() f) {
    _onClose(ptr.getInnerPtr(), f).unwrap();
  }

  /// Sets callback, invoked when a new [RemoteMediaTrack] is added to this
  /// `Connection`.
  ///
  /// Throws [RustException] if Rust returns error.
  void onRemoteTrackAdded(void Function(RemoteMediaTrack) f) {
    _onRemoteTrackAdded(ptr.getInnerPtr(), (t) {
      f(RemoteMediaTrack(NullablePointer(t)));
    }).unwrap();
  }

  /// Sets callback, invoked when a connection quality score is updated by a
  /// server.
  ///
  /// Throws [RustException] if Rust returns error.
  void onQualityScoreUpdate(void Function(int) f) {
    _onQualityScoreUpdate(ptr.getInnerPtr(), f).unwrap();
  }

  /// Drops the associated Rust struct and nulls the local [Pointer] to it.
  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
