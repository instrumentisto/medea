import 'package:flutter/cupertino.dart';
import 'package:http/http.dart' as http;
import 'dart:convert';

const API_URI = 'http://example.com';

class ControlApi {
  Future<void> createRoom(String roomId, String memberId) async {
    var pipeline = Map();
    pipeline['publish'] = {
      'kind': 'WebRtcPublishEndpoint',
      'p2p': 'Always',
      'force_relay': false,
      'audio_settings': {
        'publish_policy': 'Optional',
      },
      'video_settings': {
        'publish_policy': 'Optional',
      },
    };

    var res = await http.post(Uri.parse("$API_URI/$roomId"), body: {
      'kind': 'Room',
      'pipeline': {
        memberId: {
          'kind': 'Member',
          'credentials': {
            'plain': 'test',
          },
          'pipeline': pipeline,
          'on_join': 'grpc://127.0.0.1:9099',
          'on_leave': 'grpc://127.0.0.1:9099',
        }
      }
    });
    if (res.statusCode != 200) {
      throw Exception("Control API errored: " + res.body);
    }
  }

  Future<String> createMember(String roomId, String memberId) async {
    var controlRoom =
        jsonDecode((await http.get(Uri.parse(API_URI + '/' + roomId))).body);
    var anotherMembers = controlRoom['element']['pipeline'].keys;
    var pipeline = Map();

    var memberIds = [];
    pipeline['publish'] = {
      'kind': 'WebRtcPublishEndpoint',
      'p2p': 'Always',
      'force_relay': false,
      'audio_settings': {
        'publish_policy': 'Optional',
      },
      'video_settings': {
        'publish_policy': 'Optional',
      },
    };

    for (var anotherMember in anotherMembers) {
      var memberId = anotherMember['id'];
      memberIds.add(memberId);
      if (anotherMember['pipeline']['publish'] != null) {
        pipeline['play-' + memberId] = {
          'kind': 'WebRtcPlayEndpoint',
          'src': "local://$roomId/$memberId/publish",
          'force_relay': false,
        };
      }
    }

    var resp = await http.post(Uri.parse("$API_URI/$roomId/$memberId"), body: {
      'kind': 'Member',
      'credentials': {
        'plain': 'test',
      },
      'pipeline': pipeline,
      'on_join': 'grpc://127.0.0.1:9099',
      'on_leave': 'grpc://127.0.0.1:9099',
    });
    if (resp.statusCode != 200) {
      throw Exception("Control API errored: " + resp.body);
    }

    for (var id in memberIds) {
      var resp = await http.post(
        Uri.parse("$API_URI/$roomId/$id/play-$memberId"),
        body: {
          'kind': 'WebRtcPlayEndpoint',
          'src': "local://$roomId/$memberId/publish",
          'force_relay': false,
        },
      );
      if (resp.statusCode != 200) {
        throw Exception("Control API errored: " + resp.body);
      }
    }

    return jsonDecode(resp.body)['sids'][memberId];
  }
}
