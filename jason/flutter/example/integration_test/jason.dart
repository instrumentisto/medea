import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:medea_jason/audio_track_constraints.dart';
import 'package:medea_jason/jason.dart';
import 'package:medea_jason/kind.dart';
import 'package:medea_jason/device_video_track_constraints.dart';
import 'package:medea_jason/media_stream_settings.dart';
import 'package:medea_jason/display_video_track_constraints.dart';

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
    var jason = Jason();
    var mediaManager = jason.mediaManager();

    var devices = mediaManager.enumerateDevices();
    var tracks = mediaManager.initLocalTracks();

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
}
