import 'dart:async';
import 'dart:ffi';

import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:medea_jason/audio_track_constraints.dart';
import 'package:medea_jason/connection_handle.dart';
import 'package:medea_jason/device_video_track_constraints.dart';
import 'package:medea_jason/display_video_track_constraints.dart';
import 'package:medea_jason/ffi/exceptions.dart';
import 'package:medea_jason/ffi/foreign_value.dart';
import 'package:medea_jason/ffi/result.dart';
import 'package:medea_jason/input_device_info.dart';
import 'package:medea_jason/jason.dart';
import 'package:medea_jason/media_stream_settings.dart';
import 'package:medea_jason/reconnect_handle.dart';
import 'package:medea_jason/remote_media_track.dart';
import 'package:medea_jason/room_close_reason.dart';
import 'package:medea_jason/track_kinds.dart';
import 'package:medea_jason/util/nullable_pointer.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('Jason', (WidgetTester tester) async {
    var jason = Jason();
    var room = jason.initRoom();

    expect(() => jason.mediaManager(), returnsNormally);
    expect(() => jason.closeRoom(room), returnsNormally);
    expect(() => jason.closeRoom(room), throwsStateError);
  });

  testWidgets('MediaManager', (WidgetTester tester) async {
    final returnsLocalMediaInitException =
        dl.lookupFunction<Result Function(Handle), Result Function(Object)>(
            'returns_local_media_init_exception');
    final returnsFutureWithLocalMediaInitException =
        dl.lookupFunction<Handle Function(Handle), Object Function(Object)>(
            'returns_future_with_local_media_init_exception');
    final returnsEnumerateDevicesException =
        dl.lookupFunction<Result Function(Handle), Result Function(Object)>(
            'returns_enumerate_devices_exception');
    final returnsFutureWithEnumerateDevicesException =
        dl.lookupFunction<Handle Function(Handle), Object Function(Object)>(
            'returns_future_enumerate_devices_exception');

    var jason = Jason();
    var mediaManager = jason.mediaManager();

    var devices = await mediaManager.enumerateDevices();
    var tracks = await mediaManager.initLocalTracks(MediaStreamSettings());

    expect(devices.length, equals(3));
    expect(tracks.length, equals(3));

    expect(devices.first.ptr.getInnerPtr(),
        isNot(equals(devices.last.ptr.getInnerPtr())));
    expect(tracks.first.ptr.getInnerPtr(),
        isNot(equals(tracks.last.ptr.getInnerPtr())));

    expect(devices.first.deviceId(), equals('InputDeviceInfo.device_id'));
    expect(devices.first.groupId(), equals('InputDeviceInfo.group_id'));
    expect(devices.first.kind(), equals(MediaKind.Audio));
    expect(devices.first.label(), equals('InputDeviceInfo.label'));

    devices.first.free();
    expect(() => devices.first.label(), throwsStateError);

    expect(tracks.first.kind(), equals(MediaKind.Video));
    expect(tracks.first.mediaSourceKind(), equals(MediaSourceKind.Display));

    tracks.first.free();
    expect(() => tracks.first.kind(), throwsStateError);

    expect(
        () => returnsLocalMediaInitException('Dart err cause1').unwrap(),
        throwsA(predicate((e) =>
            e is LocalMediaInitException &&
            e.kind == LocalMediaInitExceptionKind.GetUserMediaFailed &&
            e.cause == 'Dart err cause1' &&
            e.nativeStackTrace.contains('at jason/src'))));

    var err;
    try {
      await (returnsFutureWithLocalMediaInitException('Dart err cause2')
          as Future);
    } catch (e) {
      err = e as LocalMediaInitException;
    }
    expect(
        err,
        predicate((e) =>
            e is LocalMediaInitException &&
            e.kind == LocalMediaInitExceptionKind.GetDisplayMediaFailed &&
            e.cause == 'Dart err cause2' &&
            e.nativeStackTrace.contains('at jason/src')));

    expect(
        () => returnsEnumerateDevicesException('Dart err cause3').unwrap(),
        throwsA(predicate((e) =>
            e is EnumerateDevicesException &&
            e.cause == 'Dart err cause3' &&
            e.nativeStackTrace.contains('at jason/src'))));

    var err2;
    try {
      await (returnsFutureWithEnumerateDevicesException('Dart err cause4')
          as Future);
    } catch (e) {
      err2 = e as EnumerateDevicesException;
    }
    expect(
        err2,
        predicate((e) =>
            e is EnumerateDevicesException &&
            e.cause == 'Dart err cause4' &&
            e.nativeStackTrace.contains('at jason/src')));
  });

  testWidgets('DeviceVideoTrackConstraints', (WidgetTester tester) async {
    var constraints = DeviceVideoTrackConstraints();
    constraints.deviceId('deviceId');
    constraints.exactFacingMode(FacingMode.User);
    constraints.idealFacingMode(FacingMode.Right);
    constraints.exactHeight(444);
    constraints.idealHeight(111);
    constraints.heightInRange(55, 66);
    constraints.exactWidth(444);
    constraints.idealWidth(111);
    constraints.widthInRange(55, 66);

    expect(() => constraints.exactHeight(-1), throwsArgumentError);
    expect(() => constraints.idealHeight(-1), throwsArgumentError);
    expect(() => constraints.exactHeight(1 << 32 + 1), throwsArgumentError);
    expect(() => constraints.heightInRange(-1, 200), throwsArgumentError);
    expect(() => constraints.heightInRange(200, -1), throwsArgumentError);

    expect(() => constraints.exactWidth(-1), throwsArgumentError);
    expect(() => constraints.idealWidth(-1), throwsArgumentError);
    expect(() => constraints.exactWidth(1 << 32 + 1), throwsArgumentError);
    expect(() => constraints.widthInRange(-1, 200), throwsArgumentError);
    expect(() => constraints.widthInRange(200, -1), throwsArgumentError);

    constraints.free();
    expect(() => constraints.deviceId('deviceId'), throwsStateError);

    var constraints2 = DeviceVideoTrackConstraints();
    var settings = MediaStreamSettings();
    constraints2.deviceId('deviceId');
    settings.deviceVideo(constraints2);
    expect(() => constraints2.deviceId('deviceId'), throwsStateError);
  });

  testWidgets('DisplayVideoTrackConstraints', (WidgetTester tester) async {
    var constraints = DisplayVideoTrackConstraints();
    constraints.free();
    expect(() => constraints.free(), throwsStateError);

    var constraints2 = DisplayVideoTrackConstraints();
    var settings = MediaStreamSettings();
    settings.displayVideo(constraints2);
    expect(() => settings.displayVideo(constraints2), throwsStateError);
  });

  testWidgets('AudioTrackConstraints', (WidgetTester tester) async {
    var constraints = AudioTrackConstraints();
    constraints.deviceId('deviceId');
    constraints.free();
    expect(() => constraints.deviceId('deviceId'), throwsStateError);

    var constraints2 = AudioTrackConstraints();
    var settings = MediaStreamSettings();
    constraints2.deviceId('deviceId');
    settings.audio(constraints2);
    expect(() => constraints2.deviceId('deviceId'), throwsStateError);
  });

  testWidgets('RoomHandle', (WidgetTester tester) async {
    var jason = Jason();
    var room = jason.initRoom();

    var allFired = List<Completer>.generate(4, (_) => Completer());

    room.onClose((reason) {
      allFired[0].complete();
    });

    room.onConnectionLoss((reconnectHandle) {
      allFired[1].complete();
    });

    room.onLocalTrack((localTrack) {
      allFired[2].complete();
    });

    room.onNewConnection((connection) {
      allFired[3].complete();
    });

    await Future.wait(allFired.map((e) => e.future))
        .timeout(Duration(seconds: 1));

    room.free();

    expect(() => room.onNewConnection((_) {}), throwsStateError);
  });

  testWidgets('RoomCloseReason', (WidgetTester tester) async {
    var jason = Jason();
    var room = jason.initRoom();
    var reasonFut = Completer<RoomCloseReason>();

    room.onClose((reason) {
      reasonFut.complete(reason);
    });

    var reason = await reasonFut.future.timeout(Duration(seconds: 1));

    expect(reason.reason(), equals('RpcClientUnexpectedlyDropped'));
    expect(reason.isClosedByServer(), equals(false));
    expect(reason.isErr(), equals(true));
    reason.free();
    expect(() => reason.isErr(), throwsStateError);
  });

  testWidgets('ConnectionHandle', (WidgetTester tester) async {
    var jason = Jason();
    var room = jason.initRoom();

    var connFut = Completer<ConnectionHandle>();
    room.onNewConnection((conn) {
      connFut.complete(conn);
    });
    var conn = await connFut.future;

    expect(
        () => conn.getRemoteMemberId(),
        throwsA(allOf(
            isStateError,
            predicate((e) =>
                e.message == 'ConnectionHandle is in detached state.'))));
    var allFired = List<Completer>.generate(2, (_) => Completer());
    conn.onQualityScoreUpdate((score) {
      allFired[0].complete(score);
    });
    conn.onClose(() {
      allFired[1].complete();
    });

    var res = await Future.wait(allFired.map((e) => e.future))
        .timeout(Duration(seconds: 1));
    expect(res[0], 4);
  });

  testWidgets('ConnectionHandle', (WidgetTester tester) async {
    var jason = Jason();
    var room = jason.initRoom();

    var connFut = Completer<ConnectionHandle>();
    room.onNewConnection((conn) {
      connFut.complete(conn);
    });
    var conn = await connFut.future;

    var trackFut = Completer<RemoteMediaTrack>();
    conn.onRemoteTrackAdded((remoteTrack) {
      trackFut.complete(remoteTrack);
    });

    var track = await trackFut.future;

    expect(track.enabled(), equals(true));
    expect(track.muted(), equals(false));
    expect(track.kind(), equals(MediaKind.Video));
    expect(track.mediaSourceKind(), equals(MediaSourceKind.Device));

    var allFired = List<Completer>.generate(5, (_) => Completer());
    track.onEnabled(() {
      allFired[0].complete();
    });
    track.onDisabled(() {
      allFired[1].complete();
    });
    track.onMuted(() {
      allFired[2].complete();
    });
    track.onUnmuted(() {
      allFired[3].complete();
    });
    track.onStopped(() {
      allFired[4].complete();
    });

    await Future.wait(allFired.map((e) => e.future))
        .timeout(Duration(seconds: 1));

    track.free();
    expect(() => track.kind(), throwsStateError);
  });

  testWidgets('RoomHandle', (WidgetTester tester) async {
    var jason = Jason();
    var room = jason.initRoom();

    await room.join('wss://example.com/room/Alice?token=777');
    await room.setLocalMediaSettings(MediaStreamSettings(), true, false);
    await room.muteAudio();
    await room.unmuteAudio();
    await room.muteVideo();
    await room.unmuteVideo(MediaSourceKind.Display);
    await room.disableVideo(MediaSourceKind.Display);
    await room.enableVideo(MediaSourceKind.Device);
    await room.disableAudio();
    await room.enableAudio();
    await room.disableRemoteAudio();
    await room.enableRemoteAudio();
    await room.disableRemoteVideo();

    var stateErr;
    try {
      await room.enableRemoteVideo();
    } catch (e) {
      stateErr = e;
    }
    expect(
        stateErr,
        allOf(isStateError,
            predicate((e) => e.message == 'RoomHandle is in detached state.')));

    var formatExc;
    try {
      await room.join('obviously bad url');
    } catch (e) {
      formatExc = e;
    }
    expect(
        formatExc,
        allOf(
            isFormatException,
            predicate(
                (e) => e.message.contains('relative URL without a base'))));

    var localMediaErr = Completer<Object>();
    room.onFailedLocalMedia((err) {
      localMediaErr.complete(err);
    });
    var err = await localMediaErr.future;
    expect(
        err,
        predicate((e) =>
            e is MediaStateTransitionException &&
            e.message == 'SimpleTracksRequest should have at least one track' &&
            e.nativeStackTrace.contains('at jason/src')));
  });

  testWidgets('ReconnectHandle', (WidgetTester tester) async {
    final returnsRpcClientException =
        dl.lookupFunction<Result Function(Handle), Result Function(Object)>(
            'returns_rpc_client_exception');
    final returnsFutureWithRpcClientException =
        dl.lookupFunction<Handle Function(Handle), Object Function(Object)>(
            'returns_future_rpc_client_exception');

    var jason = Jason();
    var room = jason.initRoom();

    var handleFut = Completer<ReconnectHandle>();
    room.onConnectionLoss((reconnectHandle) {
      handleFut.complete(reconnectHandle);
    });
    var handle = await handleFut.future;

    await handle.reconnectWithDelay(155);
    await handle.reconnectWithBackoff(1, 2, 3);

    var exception;
    try {
      await handle.reconnectWithDelay(-1);
    } catch (e) {
      exception = e;
    }
    expect(exception, isArgumentError);

    var exception2;
    try {
      await handle.reconnectWithBackoff(-1, 2, 3, 145);
    } catch (e) {
      exception2 = e;
    }
    expect(exception2, isArgumentError);

    var exception3;
    try {
      await handle.reconnectWithBackoff(1, 2, -3, 333);
    } catch (e) {
      exception3 = e;
    }
    expect(exception3, isArgumentError);
    var argumentError = exception3 as ArgumentError;
    expect(argumentError.invalidValue, equals(-3));
    expect(argumentError.name, 'maxDelay');
    expect(argumentError.message, 'Expected u32');

    var exception4;
    try {
      await handle.reconnectWithBackoff(1, 2, 3, -4);
    } catch (e) {
      exception4 = e;
    }
    expect(exception4, isArgumentError);
    var argumentError2 = exception4 as ArgumentError;
    expect(argumentError2.invalidValue, equals(-4));
    expect(argumentError2.name, 'maxElapsedTimeMs');
    expect(argumentError2.message, 'Expected u32');

    expect(
        () => returnsRpcClientException('Dart err cause1').unwrap(),
        throwsA(predicate((e) =>
            e is RpcClientException &&
            e.kind == RpcClientExceptionKind.ConnectionLost &&
            e.cause == 'Dart err cause1' &&
            e.message == 'RpcClientException::ConnectionLost' &&
            e.nativeStackTrace.contains('at jason/src'))));

    var exception5;
    try {
      await (returnsFutureWithRpcClientException('Dart err cause2') as Future);
    } catch (e) {
      exception5 = e;
    }
    expect(
        exception5,
        predicate((e) =>
            e is RpcClientException &&
            e.kind == RpcClientExceptionKind.SessionFinished &&
            e.message == 'RpcClientException::SessionFinished' &&
            e.cause == 'Dart err cause2' &&
            e.nativeStackTrace.contains('at jason/src')));
  });

  final returnsInputDevicePtr =
      dl.lookupFunction<ForeignValue Function(), ForeignValue Function()>(
          'returns_input_device_info_ptr');

  testWidgets('ForeignValue Rust => Dart', (WidgetTester tester) async {
    final returnsNone =
        dl.lookupFunction<ForeignValue Function(), ForeignValue Function()>(
            'returns_none');
    final returnsHandlePtr = dl.lookupFunction<ForeignValue Function(Handle),
        ForeignValue Function(Object)>('returns_handle_ptr');
    final returnsString =
        dl.lookupFunction<ForeignValue Function(), ForeignValue Function()>(
            'returns_string');
    final returnsInt =
        dl.lookupFunction<ForeignValue Function(), ForeignValue Function()>(
            'returns_int');

    expect(returnsNone().toDart(), equals(null));

    var inputDevice =
        InputDeviceInfo(NullablePointer(returnsInputDevicePtr().toDart()));
    expect(inputDevice.deviceId(), equals('InputDeviceInfo.device_id'));
    inputDevice.free();

    expect(returnsHandlePtr('asd').toDart(), equals('asd'));
    expect(returnsHandlePtr(111).toDart(), equals(111));
    expect(returnsHandlePtr(null).toDart(), equals(null));

    expect(returnsString().toDart(), equals('QWERTY'));

    expect(returnsInt().toDart(), equals(333));
  });

  testWidgets('ForeignValue Dart => Rust', (WidgetTester tester) async {
    final acceptsNone = dl.lookupFunction<Void Function(ForeignValue),
        void Function(ForeignValue)>('accepts_none');
    final acceptsPtr = dl.lookupFunction<Void Function(ForeignValue),
        void Function(ForeignValue)>('accepts_input_device_info_pointer');
    final acceptsString = dl.lookupFunction<Void Function(ForeignValue),
        void Function(ForeignValue)>('accepts_string');
    final acceptsInt = dl.lookupFunction<Void Function(ForeignValue),
        void Function(ForeignValue)>('accepts_int');

    var none = ForeignValue.none();
    var ptr =
        ForeignValue.fromPtr(NullablePointer(returnsInputDevicePtr().toDart()));
    var str = ForeignValue.fromString('my string');
    var num = ForeignValue.fromInt(235);

    acceptsNone(none.ref);
    acceptsPtr(ptr.ref);
    acceptsString(str.ref);
    acceptsInt(num.ref);

    none.free();
    ptr.free();
    str.free();
    num.free();
  });
}
