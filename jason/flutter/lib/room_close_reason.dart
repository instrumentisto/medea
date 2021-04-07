import 'dart:ffi';
import 'ffi.dart' as ffi;

final _reasonDart _reason =
    ffi.dl.lookupFunction<_reasonC, _reasonDart>('RoomCloseReason__reason');
typedef _reasonC = Pointer<Utf8> Function(Pointer);
typedef _reasonDart = Pointer<Utf8> Function(Pointer);

final _isClosedByServerDart _isClosedByServer = ffi.dl
    .lookupFunction<_isClosedByServerC, _isClosedByServerDart>(
        'RoomCloseReason__is_closed_by_server');
typedef _isClosedByServerC = bool Function(Pointer);
typedef _isClosedByServerDart = bool Function(Pointer);

final _isErrDart _isErr =
    ffi.dl.lookupFunction<_isErrC, _isErrDart>('RoomCloseReason__is_err');
typedef _isErrC = bool Function(Pointer);
typedef _isErrDart = bool Function(Pointer);

class RoomCloseReason {
  late Pointer ptr;

  RoomCloseReason(Pointer p) {
    ptr = p;
  }

  String reason() {
    return _reason(ptr).toDartString();
  }

  bool isClosedByServer() {
    return _isClosedByServer(ptr);
  }

  bool isErr() {
    return _isErr(ptr);
  }
}
