import 'dart:ffi';

import 'jason.dart';
import 'kind.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';

typedef _kind_C = Int16 Function(Pointer);
typedef _kind_Dart = int Function(Pointer);

typedef _mediaSourceKind_C = Int16 Function(Pointer);
typedef _mediaSourceKind_Dart = int Function(Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _kind_Dart _kind =
    dl.lookupFunction<_kind_C, _kind_Dart>('LocalMediaTrack__kind');
final _mediaSourceKind_Dart _sourceKind =
    dl.lookupFunction<_mediaSourceKind_C, _mediaSourceKind_Dart>(
        'LocalMediaTrack__media_source_kind');
final _free_Dart _free =
    dl.lookupFunction<_free_C, _free_Dart>('LocalMediaTrack__free');

class LocalMediaTrack {
  late NullablePointer ptr;

  LocalMediaTrack(this.ptr);

  MediaKind kind() {
    var index = _kind(ptr.getInnerPtr());
    return MediaKind.values[index];
  }

  MediaSourceKind mediaSourceKind() {
    var index = _sourceKind(ptr.getInnerPtr());
    return MediaSourceKind.values[index];
  }

  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
