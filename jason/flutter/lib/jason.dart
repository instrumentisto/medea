library jason;

import 'dart:ffi';
import 'dart:io';

import 'media_manager.dart';
import 'room_handle.dart';
import 'util/executor.dart';
import 'util/move_semantic.dart';
import 'util/nullable_pointer.dart';
import 'util/callback.dart' as callback;
import 'util/completer.dart' as completer;

typedef _new_C = Pointer Function();
typedef _new_Dart = Pointer Function();

typedef _mediaManager_C = Pointer Function(Pointer);
typedef _mediaManager_Dart = Pointer Function(Pointer);

typedef _closeRoom_C = Void Function(Pointer, Pointer);
typedef _closeRoom_Dart = void Function(Pointer, Pointer);

typedef _initRoom_C = Pointer Function(Pointer);
typedef _initRoom_Dart = Pointer Function(Pointer);

typedef _free_C = Void Function(Pointer);
typedef _free_Dart = void Function(Pointer);

final DynamicLibrary dl = _dl_load();

final _new = dl.lookupFunction<_new_C, _new_Dart>('Jason__new');

final _media_manager = dl.lookupFunction<_mediaManager_C, _mediaManager_Dart>(
    'Jason__media_manager');

final _initRoom =
    dl.lookupFunction<_initRoom_C, _initRoom_Dart>('Jason__init_room');

final _close_room =
    dl.lookupFunction<_closeRoom_C, _closeRoom_Dart>('Jason__close_room');

final _free = dl.lookupFunction<_free_C, _free_Dart>('Jason__free');

DynamicLibrary _dl_load() {
  if (!Platform.isAndroid) {
    throw UnsupportedError('This platform is not supported.');
  }
  if (NativeApi.majorVersion != 2) {
    // If the DartVM we're running on does not have the same major version as
    // this file was compiled against, refuse to initialize: the symbols are not
    // compatible.
    throw 'You are running unsupported NativeApi version.';
  }

  var dl = DynamicLibrary.open('libmedea_jason.so');

  var initResult = dl.lookupFunction<
      IntPtr Function(Pointer<Void>),
      int Function(
          Pointer<Void>)>('init_dart_api_dl')(NativeApi.initializeApiDLData);

  if (initResult != 0) {
    throw 'Failed to initialize Dart API. Code: $initResult';
  }
  callback.registerFunctions(dl);
  completer.registerFunctions(dl);

  Executor(dl);

  return dl;
}

/// General library interface.
///
/// Responsible for managing shared transports, local media and room
/// initialization.
class Jason {
  /// [Pointer] to the Rust struct backing this object.
  final NullablePointer ptr = NullablePointer(_new());

  /// Returns a [MediaManagerHandle] to the `MediaManager` of this [Jason].
  MediaManagerHandle mediaManager() {
    return MediaManagerHandle(
        NullablePointer(_media_manager(ptr.getInnerPtr())));
  }

  /// Creates a new `Room` and returns its [RoomHandle].
  RoomHandle initRoom() {
    return RoomHandle(NullablePointer(_initRoom(ptr.getInnerPtr())));
  }

  /// Closes the `Room` by the provided [RoomHandle].
  void closeRoom(@moveSemantics RoomHandle room) {
    _close_room(ptr.getInnerPtr(), room.ptr.getInnerPtr());
    room.ptr.free();
  }

  /// Drops the associated Rust struct and nulls the local [Pointer] to it.
  @moveSemantics
  void free() {
    _free(ptr.getInnerPtr());
    ptr.free();
  }
}
