import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'ffi.dart' as ffi;
import 'kind.dart';

final _kindDart _kind =
    ffi.dl.lookupFunction<_kindC, _kindDart>('InputDeviceInfo__kind');
typedef _kindC = Int16 Function(Pointer);
typedef _kindDart = int Function(Pointer);

final _sourceKindDart _sourceKind = ffi.dl
    .lookupFunction<_sourceKindC, _sourceKindDart>(
        'InputDeviceInfo__source_kind');
typedef _sourceKindC = Int16 Function(Pointer);
typedef _sourceKindDart = int Function(Pointer);

class LocalMediaTrack {
  late Pointer _ptr;

  LocalMediaTrack(Pointer ptr) {
    _ptr = ptr;
  }

  MediaKind kind() {
    return mediaKindFromInt(_kind(_ptr));
  }

  MediaSourceKind sourceKind() {
    return mediaSourceKindFromInt(_sourceKind(_ptr));
  }
}
