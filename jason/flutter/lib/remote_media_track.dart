import 'dart:ffi';

import 'jason.dart';
import 'kind.dart';
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

class RemoteMediaTrack {
  late NullablePointer ptr;

  RemoteMediaTrack(this.ptr);

  bool enabled() {
    return _enabled(ptr.getInnerPtr()) > 0;
  }

  bool muted() {
    return _muted(ptr.getInnerPtr()) > 0;
  }

  MediaKind kind() {
    var index = _kind(ptr.getInnerPtr());
    return MediaKind.values[index];
  }

  MediaSourceKind mediaSourceKind() {
    var index = _mediaSourceKind(ptr.getInnerPtr());
    return MediaSourceKind.values[index];
  }

  void onEnabled(void Function() f) {
    _onEnabled(ptr.getInnerPtr(), f);
  }

  void onDisabled(void Function() f) {
    _onDisabled(ptr.getInnerPtr(), f);
  }

  void onMuted(void Function() f) {
    _onMuted(ptr.getInnerPtr(), f);
  }

  void onUnmuted(void Function() f) {
    _onUnmuted(ptr.getInnerPtr(), f);
  }

  void onStopped(void Function() f) {
    _onStopped(ptr.getInnerPtr(), f);
  }

  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
