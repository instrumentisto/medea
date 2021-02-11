package com.jason.api;

public final class LocalMediaTrack {

    volatile long nativePtr;

    @CalledByNative
    LocalMediaTrack(long ptr) {
        this.nativePtr = ptr;
    }

    public MediaKind kind() {
        return MediaKind.fromInt(nativeKind(nativePtr));
    }

    public MediaSourceKind mediaSourceKind() {
        return MediaSourceKind.fromInt(nativeMediaSourceKind(nativePtr));
    }

    public synchronized void free() {
        if (nativePtr != 0) {
            nativeFree(nativePtr);
            nativePtr = 0;
        }
    }

    @Override
    protected void finalize() {
        free();
    }

    private static native int nativeKind(long self);

    private static native int nativeMediaSourceKind(long self);

    private static native void nativeFree(long self);
}