import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'call.dart';

class CallRoute extends StatefulWidget {
  @override
  _CallState createState() => _CallState();
}

class _CallState extends State {
  bool _videoDisabled = false;
  bool _audioDisabled = false;
  List<RTCVideoView> _videos = List.empty(growable: true);
  Call _call = Call();

  @override
  void initState() {
    _call.onNewStream((stream) {
      var renderer = RTCVideoRenderer();
      renderer.srcObject = stream;
      _videos.add(RTCVideoView(renderer));
    });
    _call.start("foobar", "foobar");
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
        appBar: AppBar(
          title: Text('Medea call demo'),
        ),
        body: Center(
            child: Stack(
          children: _videos,
        )),
        floatingActionButtonLocation: FloatingActionButtonLocation.centerDocked,
        floatingActionButton: Padding(
            padding: EdgeInsets.only(bottom: 50.0),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Padding(
                    padding: EdgeInsets.only(right: 30.0),
                    child: FloatingActionButton(
                      onPressed: () async {
                        setState(() {
                          _audioDisabled = !_audioDisabled;
                        });
                        await _call.toggleAudio(!_audioDisabled);
                      },
                      heroTag: null,
                      child: Icon(_audioDisabled ? Icons.mic : Icons.mic_off),
                    )),
                Padding(
                    padding: EdgeInsets.only(right: 30.0),
                    child: FloatingActionButton(
                      onPressed: () async {
                        setState(() {
                          _videoDisabled = !_videoDisabled;
                        });
                        await _call.toggleVideo(!_videoDisabled);
                      },
                      heroTag: null,
                      child: Icon(
                          _videoDisabled ? Icons.videocam : Icons.videocam_off),
                    )),
                FloatingActionButton(
                  onPressed: () {
                    Navigator.pop(context);
                  },
                  heroTag: null,
                  backgroundColor: Colors.red,
                  child: Icon(Icons.call_end),
                ),
              ],
            )));
  }
}
