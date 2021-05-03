import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';

class CallRoute extends StatefulWidget {
  @override
  _CallState createState() => _CallState();
}

class _CallState extends State {
  bool _videoDisabled = false;
  bool _audioDisabled = false;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
        appBar: AppBar(
          title: Text('Medea call demo'),
        ),
        body: Center(
            child: Stack(
          children: [
            Text('Joining Room...', style: TextStyle(fontSize: 16)),
          ],
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
                      onPressed: () {
                        setState(() {
                          _audioDisabled = !_audioDisabled;
                        });
                      },
                      heroTag: null,
                      child: Icon(_audioDisabled ? Icons.mic : Icons.mic_off),
                    )),
                Padding(
                    padding: EdgeInsets.only(right: 30.0),
                    child: FloatingActionButton(
                      onPressed: () {
                        setState(() {
                          _videoDisabled = !_videoDisabled;
                        });
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
