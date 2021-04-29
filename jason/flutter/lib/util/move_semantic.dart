class _MoveSemantics {
  const _MoveSemantics();
}

/// Marker annotation that signals that current operation on the item it is
/// applied to has move semantics.
///
/// Move semantics means that item is is applied to will be moved and can not be
/// used after.
///
/// It is meant to be applied to object's methods, in that case it means that
/// the object should not be used after that method call, and to method
/// arguments, which means that argument is moved in that method.
const _MoveSemantics moveSemantics = _MoveSemantics();
