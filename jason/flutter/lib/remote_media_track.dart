import 'dart:ffi';

import 'jason.dart';
import 'kind.dart';
import 'util/errors.dart';
import 'util/move_semantic.dart';

typedef _enabled_C = Uint8 Function(Pointer);
typedef _enabled_Dart = int Function(Pointer);

typedef _kind_C = Uint8 Function(Pointer);
typedef _kind_Dart = int Function(Pointer);

typedef _mediaSourceKind_C = Uint8 Function(Pointer);
typedef _mediaSourceKind_Dart = int Function(Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _enabled_Dart _enable =
    dl.lookupFunction<_enabled_C, _enabled_Dart>('RemoteMediaTrack__enabled');

final _kind_Dart _kind =
    dl.lookupFunction<_kind_C, _kind_Dart>('RemoteMediaTrack__kind');

final _mediaSourceKind_Dart _mediaSourceKind =
    dl.lookupFunction<_mediaSourceKind_C, _mediaSourceKind_Dart>(
        'RemoteMediaTrack__media_source_kind');

final _free_Dart _free =
    dl.lookupFunction<_free_C, _free_Dart>('RemoteMediaTrack__free');

class RemoteMediaTrack {
  late Pointer ptr;

  RemoteMediaTrack(Pointer p) {
    assertNonNull(p);

    ptr = p;
  }

  bool enabled() {
    assertNonNull(ptr);

    return _enable(ptr) > 0;
  }

  MediaKind kind() {
    assertNonNull(ptr);

    var index = _kind(ptr);
    return MediaKind.values[index];
  }

  MediaSourceKind mediaSourceKind() {
    assertNonNull(ptr);

    var index = _mediaSourceKind(ptr);
    return MediaSourceKind.values[index];
  }

  @moveSemantics
  void free() {
    _free(ptr);
  }
}
