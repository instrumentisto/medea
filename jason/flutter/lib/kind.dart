enum MediaKind {
  Audio,
  Video,
}

enum MediaSourceKind {
  Device,
  Display,
}

MediaKind mediaKindFromInt(int kind) {
  switch (kind) {
    case 0:
      return MediaKind.Audio;
    case 1:
      return MediaKind.Video;
  }
  throw Exception("Unknown enum variant");
}

MediaSourceKind mediaSourceKindFromInt(int sourceKind) {
  switch (sourceKind) {
    case 0:
      return MediaSourceKind.Device;
    case 1:
      return MediaSourceKind.Display;
  }
  throw Exception("Unknown enum variant");
}
