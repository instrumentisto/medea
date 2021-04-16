import 'dart:ffi';

import 'jason.dart';
import 'kind.dart';
import 'util/errors.dart';
import 'util/move_semantic.dart';

typedef _kind_C = Int16 Function(Pointer);
typedef _kind_Dart = int Function(Pointer);

typedef _sourceKindC_ = Int16 Function(Pointer);
typedef _sourceKind_Dart = int Function(Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _kind_Dart _kind =
    dl.lookupFunction<_kind_C, _kind_Dart>('LocalMediaTrack__kind');

final _sourceKind_Dart _sourceKind =
    dl.lookupFunction<_sourceKindC_, _sourceKind_Dart>(
        'LocalMediaTrack__source_kind');

final _free_Dart _free =
    dl.lookupFunction<_free_C, _free_Dart>('LocalMediaTrack__free');

class LocalMediaTrack {
  late Pointer ptr;

  LocalMediaTrack(Pointer p) {
    assertNonNull(p);

    ptr = p;
  }

  MediaKind kind() {
    assertNonNull(ptr);

    var index = _kind(ptr);
    return MediaKind.values[index];
  }

  MediaSourceKind sourceKind() {
    assertNonNull(ptr);

    var index = _sourceKind(ptr);
    return MediaSourceKind.values[index];
  }

  @moveSemantics
  void free() {
    _free(ptr);
  }
}
