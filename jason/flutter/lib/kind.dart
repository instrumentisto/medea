enum MediaKind {
  Audio,
  Video,
}

int nativeMediaKind(MediaKind kind) {
  switch (kind) {
    case MediaKind.Audio:
      return 0;
    case MediaKind.Video:
      return 1;
    default:
      throw Exception('Unreachable');
  }
}

enum MediaSourceKind {
  Device,
  Display,
}

int nativeMediaSourceKind(MediaSourceKind kind) {
  switch (kind) {
    case MediaSourceKind.Device:
      return 0;
    case MediaSourceKind.Display:
      return 1;
    default:
      throw Exception('Unreachable');
  }
}
