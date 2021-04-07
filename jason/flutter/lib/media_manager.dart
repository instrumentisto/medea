import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'ffi.dart' as ffi;
import 'local_media_track.dart';
import 'array.dart';
import 'input_device_info.dart';

final _initLocalTracksDart _initLocalTracks = ffi.dl
    .lookupFunction<_initLocalTracksC, _initLocalTracksDart>(
        'MediaManager__init_local_tracks');
typedef _initLocalTracksC = Array Function(Pointer);
typedef _initLocalTracksDart = Array Function(Pointer);

final _inputDeviceInfoDart _inputDeviceInfo = ffi.dl
    .lookupFunction<_inputDeviceInfoC, _inputDeviceInfoDart>(
        'MediaManager__input_device_info');
typedef _inputDeviceInfoC = Array Function(Pointer);
typedef _inputDeviceInfoDart = Array Function(Pointer);

class MediaManager {
  late Pointer _ptr;

  MediaManager(Pointer ptr) {
    _ptr = ptr;
  }

  List<LocalMediaTrack> initLocalTracks() {
    return _initLocalTracks(_ptr)
        .asList()
        .map((e) => new LocalMediaTrack(e))
        .toList();
  }

  List<InputDeviceInfo> enumerateDevices() {
    return _inputDeviceInfo(_ptr)
        .asList()
        .map((e) => new InputDeviceInfo(e))
        .toList();
  }
}
