package com.jason.api;


public final class ReconnectHandle {

    volatile long nativePtr;

    @CalledByNative
    ReconnectHandle(long ptr) {
        this.nativePtr = ptr;
    }

    public void reconnectWithDelay(long delatMs) throws Exception {
        nativeReconnectWithDelay(nativePtr, delatMs);
    }

    public void reconnectWithBackoff(long startingDelayMs, float multiplier, long maxDelay) throws Exception {
        nativeReconnectWithBackoff(nativePtr, startingDelayMs, multiplier, maxDelay);
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

    private static native void nativeReconnectWithDelay(long self, long maxDelay) throws Exception;

    private static native void nativeReconnectWithBackoff(long self, long startingDelayMs, float multiplier, long maxDelay) throws Exception;

    private static native void nativeFree(long self);
}