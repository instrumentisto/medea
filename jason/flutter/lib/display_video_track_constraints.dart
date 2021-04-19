import 'dart:ffi';

import 'jason.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';

typedef _new_C = Pointer Function();
typedef _new_Dart = Pointer Function();

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _new_Dart _new =
    dl.lookupFunction<_new_C, _new_Dart>('DisplayVideoTrackConstraints__new');

final _free_Dart _free = dl
    .lookupFunction<_free_C, _free_Dart>('DisplayVideoTrackConstraints__free');

class DisplayVideoTrackConstraints {
  final NullablePointer ptr = NullablePointer(_new());

  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
