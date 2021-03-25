#import "JasonPlugin.h"
#if __has_include(<jason/jason-Swift.h>)
#import <jason/jason-Swift.h>
#else
// Support project import fallback if the generated compatibility header
// is not copied when this plugin is created as a library.
// https://forums.swift.org/t/swift-static-libraries-dont-copy-generated-objective-c-header/19816
#import "jason-Swift.h"
#endif

@implementation JasonPlugin
+ (void)registerWithRegistrar:(NSObject<FlutterPluginRegistrar>*)registrar {
  [SwiftJasonPlugin registerWithRegistrar:registrar];
}
@end
