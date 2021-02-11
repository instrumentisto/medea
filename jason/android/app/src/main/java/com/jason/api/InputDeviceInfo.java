package com.jason.api;

import androidx.annotation.NonNull;

public final class InputDeviceInfo {

    volatile long nativePtr;

    @CalledByNative
    InputDeviceInfo(long ptr) {
        this.nativePtr = ptr;
    }

    public @NonNull
    String deviceId() {
        return nativeDeviceId(nativePtr);
    }

    public @NonNull
    MediaKind kind() {
        return MediaKind.fromInt(nativeKind(nativePtr));
    }

    public @NonNull
    String label() {
        return nativeLabel(nativePtr);
    }

    public @NonNull
    String groupId() {
        return nativeGroupId(nativePtr);
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

    private static native @NonNull
    String nativeDeviceId(long self);

    private static native int nativeKind(long self);

    private static native @NonNull
    String nativeLabel(long self);

    private static native @NonNull
    String nativeGroupId(long self);

    private static native void nativeFree(long self);
}
