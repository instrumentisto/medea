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
    var fut = _initLocalTracks(ptr.getInnerPtr(), caps.ptr.getInnerPtr());
    if (fut is Future) {
      var tracks = await fut;
      if (tracks is PtrArray) {
        return tracks
            .intoPointerList()
            .map((e) => LocalMediaTrack(NullablePointer(e)))
            .toList();
      } else {
        throw Exception('Future resolved with unexpected Object: ' +
            tracks.runtimeType.toString());
      }
    }
    {
      throw Exception(
          'Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  Future<List<InputDeviceInfo>> enumerateDevices() async {
    var fut = _enumerateDevices(ptr.getInnerPtr());
    if (fut is Future) {
      var devices = await fut;
      if (devices is PtrArray) {
        return devices
            .intoPointerList()
            .map((e) => InputDeviceInfo(NullablePointer(e)))
            .toList();
      } else {
        throw Exception('Future resolved with unexpected Object: ' +
            devices.runtimeType.toString());
      }
    }
    {
      throw Exception(
          'Unexpected Object instead of Future: ' + fut.runtimeType.toString());
    }
  }

  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
