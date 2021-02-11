package com.jason.api;

public enum MediaKind {
    Audio,
    Video;

    // TODO: dont throw?
    @CalledByNative
    static MediaKind fromInt(int x) {
        switch (x) {
            case 0:
                return Audio;
            case 1:
                return Video;
            default:
                throw new Error("Invalid value for enum MediaKind: " + x);
        }
    }
}
