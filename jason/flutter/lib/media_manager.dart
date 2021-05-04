import 'dart:ffi';

import 'input_device_info.dart';
import 'jason.dart';
import 'local_media_track.dart';
import 'media_stream_settings.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';
import 'util/ptrarray.dart';

typedef _initLocalTracks_C = Handle Function(Pointer, Pointer);
typedef _initLocalTracks_Dart = Object Function(Pointer, Pointer);

typedef _enumerateDevices_C = Handle Function(Pointer);
typedef _enumerateDevices_Dart = Object Function(Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _initLocalTracks =
    dl.lookupFunction<_initLocalTracks_C, _initLocalTracks_Dart>(
        'MediaManagerHandle__init_local_tracks');

final _enumerateDevices =
    dl.lookupFunction<_enumerateDevices_C, _enumerateDevices_Dart>(
        'MediaManagerHandle__enumerate_devices');

final _free =
    dl.lookupFunction<_free_C, _free_Dart>('MediaManagerHandle__free');

class MediaManager {
  late NullablePointer ptr;

  MediaManager(this.ptr);

  Future<List<LocalMediaTrack>> initLocalTracks(
      MediaStreamSettings caps) async {
    PtrArray tracks =
        await (_initLocalTracks(ptr.getInnerPtr(), caps.ptr.getInnerPtr())
            as Future);
    return tracks
        .intoPointerList()
        .map((e) => LocalMediaTrack(NullablePointer(e)))
        .toList();
  }

  Future<List<InputDeviceInfo>> enumerateDevices() async {
    var fut = _enumerateDevices(ptr.getInnerPtr()) as Future;
    PtrArray devices = await fut;
    return devices
        .intoPointerList()
        .map((e) => InputDeviceInfo(NullablePointer(e)))
        .toList();
  }

  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
