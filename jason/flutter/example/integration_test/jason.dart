import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:medea_jason/jason.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('add test', (WidgetTester tester) async {
    expect(add(2, 2), equals(4));
  });
}
