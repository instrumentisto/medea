import 'dart:ffi';

import 'callback.dart' as callback;
import 'future.dart' as future;
import 'list.dart' as list;
import 'map.dart' as map;
import 'option.dart' as option;
import 'panic_exception.dart' as panic_exception;
import 'error.dart' as error;

void registerFunctions(DynamicLibrary dl) {
  error.registerFunctions(dl);
  callback.registerFunctions(dl);
  future.registerFunctions(dl);
  list.registerFunctions(dl);
  map.registerFunctions(dl);
  option.registerFunctions(dl);
  panic_exception.registerFunctions(dl);
}
