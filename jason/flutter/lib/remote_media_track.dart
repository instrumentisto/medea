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

final _enabled =
    dl.lookupFunction<_enabled_C, _enabled_Dart>('RemoteMediaTrack__enabled');

final _muted =
    dl.lookupFunction<_muted_C, _muted_Dart>('RemoteMediaTrack__muted');

final _kind = dl.lookupFunction<_kind_C, _kind_Dart>('RemoteMediaTrack__kind');

final _mediaSourceKind =
    dl.lookupFunction<_mediaSourceKind_C, _mediaSourceKind_Dart>(
        'RemoteMediaTrack__media_source_kind');

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

  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
