package com.jason.api;


public enum MediaSourceKind {
    Device,
    Display;

    // TODO: dont throw?
    @CalledByNative
    static MediaSourceKind fromInt(int x) {
        switch (x) {
            case 0:
                return Device;
            case 1:
                return Display;
            default:
                throw new Error("Invalid value for enum MediaSourceKind: " + x);
        }
    }
}
