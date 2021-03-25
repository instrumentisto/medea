import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:jason/jason.dart';

void main() {
  const MethodChannel channel = MethodChannel('jason');

  TestWidgetsFlutterBinding.ensureInitialized();

  setUp(() {
    channel.setMockMethodCallHandler((MethodCall methodCall) async {
      return '42';
    });
  });

  tearDown(() {
    channel.setMockMethodCallHandler(null);
  });

  test('getPlatformVersion', () async {
    expect(await Jason.platformVersion, '42');
  });
}
