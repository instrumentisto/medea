package com.jason.api;

import androidx.annotation.NonNull;

public final class MediaManagerHandle {

    volatile long nativePtr;

    @CalledByNative
    MediaManagerHandle(long ptr) {
        this.nativePtr = ptr;
    }

    public @NonNull
    InputDeviceInfo[] enumerateDevices() throws Exception {
        return nativeEnumerateDevices(nativePtr);
    }

    public @NonNull
    LocalMediaTrack[] initLocalTracks(@NonNull MediaStreamSettings caps) throws Exception {
        LocalMediaTrack[] tracks = nativeInitLocalTracks(nativePtr, caps.nativePtr);

        ReachabilityFence.reachabilityFence(caps);

        return tracks;
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
    InputDeviceInfo[] nativeEnumerateDevices(long self) throws Exception;

    private static native @NonNull
    LocalMediaTrack[] nativeInitLocalTracks(long self, long caps) throws Exception;

    private static native void nativeFree(long self);
}