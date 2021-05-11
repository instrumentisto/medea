import 'package:flutter/material.dart';
import 'call_route.dart';
import 'join_route.dart';

void main() {
  runApp(MaterialApp(
    title: 'Medea demo',
    initialRoute: '/',
    routes: {
      '/': (context) => JoinRoute(),
      '/call': (context) => CallRoute(),
    },
  ));
}
