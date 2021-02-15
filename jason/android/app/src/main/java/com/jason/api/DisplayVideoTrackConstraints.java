package com.jason.api;


public final class DisplayVideoTrackConstraints {

    volatile long nativePtr;

    @CalledByNative
    DisplayVideoTrackConstraints(long ptr) {
        this.nativePtr = ptr;
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

    private static native void nativeFree(long self);
}
