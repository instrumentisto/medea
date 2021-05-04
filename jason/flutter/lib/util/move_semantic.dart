class _MoveSemantics {
  const _MoveSemantics();
}

/// Marker annotation signalling that current operation on an item (it's applied
/// to) has move semantics.
///
/// Move semantics means that the item will be moved and can not be used after.
///
/// When applied to an object method, means that the object should not be used
/// after that method is called.
///
/// When applied to method arguments, means that the argument is moved into that
/// method.
const _MoveSemantics moveSemantics = _MoveSemantics();
