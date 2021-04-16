import 'dart:ffi';

import 'input_device_info.dart';
import 'jason.dart';
import 'local_media_track.dart';
import 'util/errors.dart';
import 'util/move_semantic.dart';
import 'util/ptrarray.dart';

typedef _initLocalTracks_C = PtrArray Function(Pointer);
typedef _initLocalTracks_Dart = PtrArray Function(Pointer);

typedef _enumerateDevices_C = PtrArray Function(Pointer);
typedef _enumerateDevices_Dart = PtrArray Function(Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _initLocalTracks_Dart _initLocalTracks =
    dl.lookupFunction<_initLocalTracks_C, _initLocalTracks_Dart>(
        'MediaManagerHandle__init_local_tracks');

final _enumerateDevices_Dart _enumerateDevices =
    dl.lookupFunction<_enumerateDevices_C, _enumerateDevices_Dart>(
        'MediaManagerHandle__enumerate_devices');

final _free_Dart _free =
    dl.lookupFunction<_free_C, _free_Dart>('MediaManagerHandle__free');

class MediaManager {
  late Pointer ptr;

  MediaManager(Pointer p) {
    assertNonNull(p);

    ptr = p;
  }

  List<LocalMediaTrack> initLocalTracks() {
    assertNonNull(ptr);

    return _initLocalTracks(ptr)
        .intoList()
        .map((e) => LocalMediaTrack(e))
        .toList();
  }

  List<InputDeviceInfo> enumerateDevices() {
    assertNonNull(ptr);

    return _enumerateDevices(ptr)
        .intoList()
        .map((e) => InputDeviceInfo(e))
        .toList();
  }

  @moveSemantics
  void free() {
    _free(ptr);
  }
}
