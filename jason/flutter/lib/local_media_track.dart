import 'dart:ffi';

import 'jason.dart';
import 'track_kinds.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';

typedef _kind_C = Uint8 Function(Pointer);
typedef _kind_Dart = int Function(Pointer);

typedef _mediaSourceKind_C = Uint8 Function(Pointer);
typedef _mediaSourceKind_Dart = int Function(Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final _kind = dl.lookupFunction<_kind_C, _kind_Dart>('LocalMediaTrack__kind');

final _sourceKind =
    dl.lookupFunction<_mediaSourceKind_C, _mediaSourceKind_Dart>(
        'LocalMediaTrack__media_source_kind');

final _free = dl.lookupFunction<_free_C, _free_Dart>('LocalMediaTrack__free');

/// Strongly referenced media track received from a
/// [getUserMedia()][1]/[getDisplayMedia()][2] request.
///
/// [1]: https://w3.org/TR/mediacapture-streams#dom-mediadevices-getusermedia
/// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
class LocalMediaTrack {
  /// [Pointer] to Rust struct that backs this object.
  late NullablePointer ptr;

  /// Constructs new [LocalMediaTrack] backed by Rust object behind provided
  /// [Pointer].
  LocalMediaTrack(this.ptr);

  /// Returns a [MediaKind.Audio] if this [LocalMediaTrack] represents an
  /// audio track, or a [MediaKind.Video] if it represents a video track.
  MediaKind kind() {
    var index = _kind(ptr.getInnerPtr());
    return MediaKind.values[index];
  }

  /// Returns a [MediaSourceKind.Device] if this [LocalMediaTrack] is sourced
  /// from some device (webcam/microphone), or a [MediaSourceKind.Display]
  /// if it's captured via [MediaDevices.getDisplayMedia()][1].
  ///
  /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
  MediaSourceKind mediaSourceKind() {
    var index = _sourceKind(ptr.getInnerPtr());
    return MediaSourceKind.values[index];
  }

  /// Drops associated Rust object and nulls the local [Pointer] to this object.
  ///
  /// Note that this is a strong reference, so freeing it will stop underlying
  /// track if there are no other strong references (it is not used in local
  /// peer's senders).
  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
