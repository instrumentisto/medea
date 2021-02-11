package com.jason.api;

import androidx.annotation.NonNull;

public final class DeviceVideoTrackConstraints {

    volatile long nativePtr;

    @CalledByNative
    DeviceVideoTrackConstraints(long ptr) {
        this.nativePtr = ptr;
    }

    public void deviceId(@NonNull String deviceId) {
        nativeDeviceId(nativePtr, deviceId);
    }

    public void exactFacingMode(@NonNull FacingMode facingMode) {
        nativeExactFacingMode(nativePtr, facingMode.ordinal());
    }

    public void idealFacingMode(@NonNull FacingMode facingMode) {
        nativeIdealFacingMode(nativePtr, facingMode.ordinal());
    }

    public void exactHeight(long height) {
        nativeExactHeight(nativePtr, height);
    }

    public void idealHeight(long height) {
        nativeIdealHeight(nativePtr, height);
    }

    public void heightInRange(long min, long max) {
        nativeHeightInRange(nativePtr, min, max);
    }

    public void exactWidth(long width) {
        nativeExactWidth(nativePtr, width);
    }

    public void idealWidth(long width) {
        nativeIdealWidth(nativePtr, width);
    }

    public void widthInRange(long min, long max) {
        nativeWidthInRange(nativePtr, min, max);
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

    private static native void nativeExactFacingMode(long self, int facingMode);

    private static native void nativeIdealFacingMode(long self, int facingMode);

    private static native void nativeExactHeight(long self, long height);

    private static native void nativeIdealHeight(long self, long height);

    private static native void nativeHeightInRange(long self, long min, long max);

    private static native void nativeExactWidth(long self, long width);

    private static native void nativeIdealWidth(long self, long width);

    private static native void nativeWidthInRange(long self, long min, long max);

    private static native void nativeFree(long self);
}