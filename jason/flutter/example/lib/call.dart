import 'dart:collection';

import 'package:medea_jason/audio_track_constraints.dart';
import 'package:medea_jason/device_video_track_constraints.dart';
import 'package:medea_jason/jason.dart';
import 'package:medea_jason/media_stream_settings.dart';
import 'package:medea_jason/room_handle.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:medea_jason/track_kinds.dart';

class Call {
  Jason _jason = Jason();
  late RoomHandle _room;
  HashMap<String, MediaStream> _remoteStreams = HashMap();

  Call() {
    _room = _jason.initRoom();
  }

  Future<void> start(String roomId, String username) async {
    var constraints = _buildConstraints();
    await _room.setLocalMediaSettings(constraints, false, false);
  }

  void onNewStream(Function(MediaStream) f) {
    _room.onNewConnection((conn) {
      var remoteMemberId = conn.getRemoteMemberId();
      conn.onRemoteTrackAdded((track) async {
        var sysTrack = track.getTrack();
        if (_remoteStreams[remoteMemberId] != null) {
          await _remoteStreams[remoteMemberId]!.addTrack(sysTrack);
        } else {
          // TODO: check difference between local MediaStream and remote MediaStream.
          var remoteStream = await createLocalMediaStream(remoteMemberId);
          await remoteStream.addTrack(sysTrack);
          f(remoteStream);
        }
      });
    });
  }

  Future<void> toggleAudio(bool enabled) async {
    if (enabled) {
      await _room.enableAudio();
    } else {
      await _room.disableAudio();
    }
  }

  Future<void> toggleVideo(bool enabled) async {
    if (enabled) {
      await _room.enableVideo(MediaSourceKind.Device);
    } else {
      await _room.disableVideo(MediaSourceKind.Device);
    }
  }

  MediaStreamSettings _buildConstraints() {
    var constraints = MediaStreamSettings();
    constraints.audio(AudioTrackConstraints());
    constraints.deviceVideo(DeviceVideoTrackConstraints());

    return constraints;
  }
}
