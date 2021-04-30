import 'dart:ffi';

/// Wrapper around a [Pointer] that can be null when its pointed memory is
/// freed.
///
/// Accessing the pointer after it's freed will throw [StateError].
class NullablePointer {
  Pointer? _ptr;

  /// Constructs [NullablePointer] from the provided [Pointer].
  ///
  /// Provided [Pointer] should not have a zero address.
  NullablePointer(Pointer ptr) {
    if (ptr.address == 0) {
      throw ArgumentError.notNull('ptr');
    }
    _ptr = ptr;
  }

  /// Returns the underlying [Pointer].
  ///
  /// Throws [StateError] if the underlying [Pointer] has been freed.
  Pointer getInnerPtr() {
    if (_ptr == null) {
      throw StateError('NullablePointer cannot be used after free.');
    } else {
      return Pointer.fromAddress(_ptr!.address);
    }
  }

  /// Nulls the underlying [Pointer].
  ///
  /// This doesn't affect the pointed memory, but Dart won't be able to access
  /// it, so it's expected that native memory has been freed at this point.
  void free() {
    _ptr = null;
  }
}
