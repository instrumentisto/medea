import 'package:flutter/material.dart';
import 'package:medea_jason/jason.dart';

void main() {
  runApp(MyApp());
}

class MyApp extends StatefulWidget {
  @override
  _MyAppState createState() => _MyAppState();
}

class _MyAppState extends State<MyApp> {
  int _sum = 0;

  @override
  void initState() {
    super.initState();
    _sum = add(2, 2);
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: Scaffold(
        appBar: AppBar(
          title: const Text('Plugin example app'),
        ),
        body: Center(
          child: Text('2 + 2 = $_sum\n'),
        ),
      ),
    );
  }
}
