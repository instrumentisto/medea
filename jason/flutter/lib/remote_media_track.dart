import 'dart:ffi';
import 'ffi.dart' as ffi;
import 'kind.dart';

final _enableDart _enable =
    ffi.dl.lookupFunction<_enableC, _enableDart>('RemoteMediaTrack__enable');
typedef _enableC = Void Function(Pointer);
typedef _enableDart = void Function(Pointer);

final _kindDart _kind =
    ffi.dl.lookupFunction<_kindC, _kindDart>('InputDeviceInfo__kind');
typedef _kindC = Int16 Function(Pointer);
typedef _kindDart = int Function(Pointer);

final _mediaSourceKindDart _mediaSourceKind = ffi.dl
    .lookupFunction<_mediaSourceKindC, _mediaSourceKindDart>(
        'RemoteMediaTrack__media_source_kind');
typedef _mediaSourceKindC = Int32 Function(Pointer);
typedef _mediaSourceKindDart = int Function(Pointer);

class RemoteMediaTrack {
  late Pointer ptr;

  RemoteMediaTrack(Pointer p) {
    ptr = p;
  }

  void enable() {
    _enable(ptr);
  }

  MediaKind kind() {
    return mediaKindFromInt(_kind(ptr));
  }

  MediaSourceKind mediaSourceKind() {
    return mediaSourceKindFromInt(_mediaSourceKind(ptr));
  }
}
