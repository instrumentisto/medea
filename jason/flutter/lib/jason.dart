library jason;

import 'dart:ffi';
import 'dart:io';
import 'media_manager.dart';
import 'room_handle.dart';
import 'util/errors.dart';
import 'util/move_semantic.dart';

typedef _init_C = Pointer Function();
typedef _init_Dart = Pointer Function();

typedef _media_manager_C = Pointer Function(Pointer);
typedef _media_manager_Dart = Pointer Function(Pointer);

typedef _close_room_C = Void Function(Pointer, Pointer);
typedef _close_room_Dart = void Function(Pointer, Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final DynamicLibrary dl = _dl_load();

final _init_Dart _init = dl.lookupFunction<_init_C, _init_Dart>('Jason__new');
final _media_manager_Dart _media_manager =
    dl.lookupFunction<_media_manager_C, _media_manager_Dart>(
        'Jason__media_manager');
final _close_room_Dart _close_room =
    dl.lookupFunction<_close_room_C, _close_room_Dart>('Jason__close_room');
final _free_Dart _free = dl.lookupFunction<_free_C, _free_Dart>('Jason__free');

DynamicLibrary _dl_load() {
  if (Platform.isAndroid) return DynamicLibrary.open('libjason.so');
  throw UnsupportedError('This platform is not supported.');
}

class Jason {
  final Pointer ptr = _init();

  MediaManager mediaManager() {
    assertNonNull(ptr);

    return MediaManager(_media_manager(ptr));
  }

  void closeRoom(@moveSemantics RoomHandle room) {
    assertNonNull(ptr);
    assertNonNull(room.ptr);

    _close_room(ptr, room.ptr);
  }

  @moveSemantics
  void free() {
    _free(ptr);
  }
}
