import 'package:flutter/material.dart';
import 'call_route.dart';

class JoinRoute extends StatefulWidget {
  @override
  createState() => _JoinRouteState();
}

class _JoinRouteState extends State<JoinRoute> {
  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Jason demo'),
      ),
      body: Center(
          child: Container(
              padding: EdgeInsets.all(20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Image.asset('assets/images/logo.png', height: 200),
                  TextField(
                    decoration: InputDecoration(
                      hintText: 'Room ID',
                    ),
                  ),
                  TextField(
                    decoration: InputDecoration(
                      hintText: 'Username',
                    ),
                  ),
                  TextField(
                    obscureText: true,
                    decoration: InputDecoration(
                      hintText: 'Password',
                    ),
                  ),
                  TextButton(
                    child: Text('Join Room'),
                    style: TextButton.styleFrom(
                      primary: Colors.white,
                      backgroundColor: Colors.blue,
                      onSurface: Colors.grey,
                    ),
                    onPressed: () {
                      Navigator.push(context,
                          MaterialPageRoute(builder: (context) => CallRoute()));
                    },
                  )
                ],
              ))),
    );
  }
}
