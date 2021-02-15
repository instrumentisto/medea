package com.jason.api;

import androidx.annotation.NonNull;

public final class AudioTrackConstraints {

    volatile long nativePtr;

    @CalledByNative
    AudioTrackConstraints(long ptr) {
        this.nativePtr = ptr;
    }

    public void deviceId(@NonNull String deviceId) {
        nativeDeviceId(nativePtr, deviceId);
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

    private static native void nativeDeviceId(long self, @NonNull String deviceId);

    private static native void nativeFree(long self);
}
