// Trampolines to Dynamically Linked Dart API.
//
// Trampolines allow to call Dynamically Linked Dart API from Rust.
//
// This must be compiled and linked into final library, so Rust can call these
// methods.
//
// All declared methods are simply calling Dart DL API methods with same name
// (without *_Trampolined prefix).

#include <stdlib.h>
#include "./include/dart_api_dl.c"

Dart_PersistentHandle Dart_NewPersistentHandle_DL_Trampolined(
    Dart_Handle handle) {
  return Dart_NewPersistentHandle_DL(handle);
}

Dart_Handle Dart_HandleFromPersistent_DL_Trampolined(
    Dart_PersistentHandle handle) {
  return Dart_HandleFromPersistent_DL(handle);
}

void Dart_DeletePersistentHandle_DL_Trampolined(Dart_PersistentHandle handle) {
  Dart_DeletePersistentHandle_DL(handle);
}
