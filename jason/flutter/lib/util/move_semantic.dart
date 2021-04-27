/// Marker annotation that signals that item it is applied to has move
/// semantics, meaning that it will be moved and can not be used after.
class MoveSemantics {
  const MoveSemantics();
}

const MoveSemantics moveSemantics = MoveSemantics();
