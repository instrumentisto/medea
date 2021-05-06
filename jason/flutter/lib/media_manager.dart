import 'dart:ffi';

import 'input_device_info.dart';
import 'jason.dart';
import 'local_media_track.dart';
import 'media_stream_settings.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';
import 'util/ptrarray.dart';

typedef _initLocalTracks_C = PtrArray Function(Pointer, Pointer);
typedef _initLocalTracks_Dart = PtrArray Function(Pointer, Pointer);

typedef _enumerateDevices_C = PtrArray Function(Pointer);
typedef _enumerateDevices_Dart = PtrArray Function(Pointer);

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

/// External handle to a `MediaManager`.
///
/// `MediaManager` performs all media acquisition requests
/// ([`getUserMedia()`][1]/[`getDisplayMedia()`][2]) and stores all received
/// tracks for further re-usage.
///
/// [1]: https://w3.org/TR/mediacapture-streams#dom-mediadevices-getusermedia
/// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
class MediaManagerHandle {
  /// [Pointer] to the Rust struct backing this object.
  late NullablePointer ptr;

  /// Creates a new [MediaManagerHandle] backed by the Rust struct behind the
  /// provided [Pointer].
  MediaManagerHandle(this.ptr);

  /// Obtains [LocalMediaTrack]s objects from local media devices (or screen
  /// capture) basing on the provided [MediaStreamSettings].
  List<LocalMediaTrack> initLocalTracks(MediaStreamSettings caps) {
    return _initLocalTracks(ptr.getInnerPtr(), caps.ptr.getInnerPtr())
        .intoPointerList()
        .map((e) => LocalMediaTrack(NullablePointer(e)))
        .toList();
  }

  /// Returns a list of [InputDeviceInfo] objects representing available media
  /// input devices, such as microphones, cameras, and so forth.
  List<InputDeviceInfo> enumerateDevices() {
    return _enumerateDevices(ptr.getInnerPtr())
        .intoPointerList()
        .map((e) => InputDeviceInfo(NullablePointer(e)))
        .toList();
  }

  /// Drops the associated Rust struct and nulls the local [Pointer] to it.
  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
