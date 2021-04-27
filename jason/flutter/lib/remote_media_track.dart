import 'dart:ffi';

import 'jason.dart';
import 'track_kinds.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';

typedef _enabled_C = Uint8 Function(Pointer);
typedef _enabled_Dart = int Function(Pointer);

typedef _muted_C = Uint8 Function(Pointer);
typedef _muted_Dart = int Function(Pointer);

typedef _kind_C = Uint8 Function(Pointer);
typedef _kind_Dart = int Function(Pointer);

typedef _mediaSourceKind_C = Uint8 Function(Pointer);
typedef _mediaSourceKind_Dart = int Function(Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

typedef _onEnabled_C = Void Function(Pointer, Handle);
typedef _onEnabled_Dart = void Function(Pointer, void Function());

typedef _onDisabled_C = Void Function(Pointer, Handle);
typedef _onDisabled_Dart = void Function(Pointer, void Function());

typedef _onMuted_C = Void Function(Pointer, Handle);
typedef _onMuted_Dart = void Function(Pointer, void Function());

typedef _onUnmuted_C = Void Function(Pointer, Handle);
typedef _onUnmuted_Dart = void Function(Pointer, void Function());

typedef _onStopped_C = Void Function(Pointer, Handle);
typedef _onStopped_Dart = void Function(Pointer, void Function());

final _enabled =
    dl.lookupFunction<_enabled_C, _enabled_Dart>('RemoteMediaTrack__enabled');

final _muted =
    dl.lookupFunction<_muted_C, _muted_Dart>('RemoteMediaTrack__muted');

final _kind = dl.lookupFunction<_kind_C, _kind_Dart>('RemoteMediaTrack__kind');

final _mediaSourceKind =
    dl.lookupFunction<_mediaSourceKind_C, _mediaSourceKind_Dart>(
        'RemoteMediaTrack__media_source_kind');

final _onEnabled = dl.lookupFunction<_onEnabled_C, _onEnabled_Dart>(
    'RemoteMediaTrack__on_enabled');

final _onDisabled = dl.lookupFunction<_onDisabled_C, _onDisabled_Dart>(
    'RemoteMediaTrack__on_disabled');

final _onMuted =
    dl.lookupFunction<_onMuted_C, _onMuted_Dart>('RemoteMediaTrack__on_muted');

final _onUnmuted = dl.lookupFunction<_onUnmuted_C, _onUnmuted_Dart>(
    'RemoteMediaTrack__on_unmuted');

final _onStopped = dl.lookupFunction<_onStopped_C, _onStopped_Dart>(
    'RemoteMediaTrack__on_stopped');

final _free = dl.lookupFunction<_free_C, _free_Dart>('RemoteMediaTrack__free');

/// Wrapper around a received remote [MediaStreamTrack][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams#dom-mediastreamtrack
class RemoteMediaTrack {
  /// [Pointer] to Rust struct that backs this object.
  late NullablePointer ptr;

  /// Constructs new [RemoteMediaTrack] backed by Rust object behind provided
  /// [Pointer].
  RemoteMediaTrack(this.ptr);

  /// Indicates whether this [RemoteMediaTrack] is enabled.
  bool enabled() {
    return _enabled(ptr.getInnerPtr()) > 0;
  }

  /// Indicate whether this [RemoteMediaTrack] is muted.
  bool muted() {
    return _muted(ptr.getInnerPtr()) > 0;
  }

  /// Returns this [RemoteMediaTrack]'s kind (audio/video).
  MediaKind kind() {
    var index = _kind(ptr.getInnerPtr());
    return MediaKind.values[index];
  }

  /// Returns this [RemoteMediaTrack]'s media source kind.
  MediaSourceKind mediaSourceKind() {
    var index = _mediaSourceKind(ptr.getInnerPtr());
    return MediaSourceKind.values[index];
  }

  /// Sets callback, invoked when this [RemoteMediaTrack] is enabled.
  void onEnabled(void Function() f) {
    _onEnabled(ptr.getInnerPtr(), f);
  }

  /// Sets callback, invoked when this [RemoteMediaTrack] is disabled.
  void onDisabled(void Function() f) {
    _onDisabled(ptr.getInnerPtr(), f);
  }

  /// Sets callback to invoke when this [RemoteMediaTrack] is muted.
  void onMuted(void Function() f) {
    _onMuted(ptr.getInnerPtr(), f);
  }

  /// Sets callback to invoke when this [RemoteMediaTrack] is unmuted.
  void onUnmuted(void Function() f) {
    _onUnmuted(ptr.getInnerPtr(), f);
  }

  /// Sets callback to invoke when this [RemoteMediaTrack] is stopped.
  void onStopped(void Function() f) {
    _onStopped(ptr.getInnerPtr(), f);
  }

  /// Drops associated Rust object and nulls the local [Pointer] to this object.
  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
