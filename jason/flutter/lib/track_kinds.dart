/// Representation of a [`MediaStreamTrack.kind`][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams#dom-mediastreamtrack-kind
enum MediaKind {
  /// Audio track.
  Audio,

  /// Video track.
  Video,
}

/// Media source type.
enum MediaSourceKind {
  /// Media is sourced from some media device (webcam or microphone).
  Device,

  /// Media is obtained via screen capturing.
  Display,
}
