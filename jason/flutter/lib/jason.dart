library jason;

import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'ffi.dart' as ffi;
import 'media_manager.dart';

final _init_Dart _init =
    ffi.dl.lookupFunction<_init_C, _init_Dart>("Jason__init");
typedef _init_C = Pointer Function();
typedef _init_Dart = Pointer Function();

final _media_manager_Dart _media_manager = ffi.dl
    .lookupFunction<_media_manager_C, _media_manager_Dart>(
        'Jason__media_manager');
typedef _media_manager_C = Pointer Function(Pointer);
typedef _media_manager_Dart = Pointer Function(Pointer);

final _close_room_Dart _close_room =
    ffi.dl.lookupFunction<_close_room_C, _close_room_Dart>('Jason__close_room');
typedef _close_room_C = Void Function(Pointer);
typedef _close_room_Dart = void Function(Pointer);

class Jason {
  final Pointer _ptr = _init();

  MediaManager mediaManager() {
    return new MediaManager(_media_manager(_ptr));
  }

  void closeRoom() {
    _close_room(_ptr);
  }
}
