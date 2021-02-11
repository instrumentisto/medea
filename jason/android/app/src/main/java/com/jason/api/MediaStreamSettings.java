package com.jason.api;

import androidx.annotation.NonNull;

public final class MediaStreamSettings {

    volatile long nativePtr;

    @CalledByNative
    MediaStreamSettings(long ptr) {
        this.nativePtr = ptr;
    }

    @MoveSemantics
    public void audio(@NonNull AudioTrackConstraints constraints) {
        long constraintsPtr = constraints.nativePtr;
        constraints.nativePtr = 0;

        nativeAudio(nativePtr, constraintsPtr);

        ReachabilityFence.reachabilityFence(constraints);
    }

    @MoveSemantics
    public void deviceVideo(@NonNull DeviceVideoTrackConstraints constraints) {
        long constraintsPtr = constraints.nativePtr;
        constraints.nativePtr = 0;

        nativeDeviceVideo(nativePtr, constraintsPtr);

        ReachabilityFence.reachabilityFence(constraints);
    }

    @MoveSemantics
    public void displayVideo(@NonNull DisplayVideoTrackConstraints constraints) {
        long constraintsPtr = constraints.nativePtr;
        constraints.nativePtr = 0;

        nativeDisplayVideo(nativePtr, constraintsPtr);

        ReachabilityFence.reachabilityFence(constraints);
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

    private static native void nativeAudio(long self, long constraints);

    private static native void nativeDeviceVideo(long self, long constraints);

    private static native void nativeDisplayVideo(long self, long constraints);

    private static native void nativeFree(long self);
}