import Flutter
import UIKit

public class SwiftJasonPlugin: NSObject, FlutterPlugin {
  public static func register(with registrar: FlutterPluginRegistrar) {
  }

  public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    // Noop
    result(nil)
  }

  /// This is necessary so that the Swift compiler does not remove the dynamic
  /// library from the final application.
  public static func dummyMethodToEnforceBundling() {
    dummy_function();
  }
}
