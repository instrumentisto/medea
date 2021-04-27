import 'dart:ffi';

/// Wrapper for [Pointer] that can be null. Accessing pointer after it is freed
/// will throw [StateError].
class NullablePointer {
  Pointer? _ptr;

  /// Constructs [NullablePointer] from the provided [Pointer].
  ///
  /// Provided [Pointer] should not have zero address.
  NullablePointer(Pointer ptr) {
    if (ptr.address == 0) {
      throw ArgumentError.notNull('ptr');
    }
    _ptr = ptr;
  }

  /// Returns underlying [Pointer].
  ///
  /// Throws [StateError] if underlying [Pointer] was freed.
  Pointer getInnerPtr() {
    if (_ptr == null) {
      throw StateError('NullablePointer cannot be used after free.');
    } else {
      return Pointer.fromAddress(_ptr!.address);
    }
  }

  /// Nulls underlying [Pointer].
  ///
  /// This does not affect data behind [Pointer], but Dart won't be able to
  /// access it, so it is expected that native memory was freed at this point.
  void free() {
    _ptr = null;
  }
}
