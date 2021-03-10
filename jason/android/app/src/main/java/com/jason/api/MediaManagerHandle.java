package com.jason.api;

import androidx.annotation.NonNull;

import java.util.Arrays;
import java.util.List;

import static java.util.stream.Collectors.toList;

public final class MediaManagerHandle {

    volatile long nativePtr;

    @CalledByNative
    MediaManagerHandle(long ptr) {
        this.nativePtr = ptr;
    }

    public @NonNull
    InputDeviceInfo[] enumerateDevices() throws Exception {
        long[] devices = nativeEnumerateDevices(nativePtr);
        InputDeviceInfo[] output = new InputDeviceInfo[devices.length];

        for (int i = 0; i < devices.length; i++) {
           output[i] = new InputDeviceInfo(devices[i]);
        }

        return output;
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
    long[] nativeEnumerateDevices(long self) throws Exception;

    private static native @NonNull
    LocalMediaTrack[] nativeInitLocalTracks(long self, long caps) throws Exception;

    private static native void nativeFree(long self);
}
