package com.jason.api;

import androidx.annotation.NonNull;

public final class RoomCloseReason {

    volatile long nativePtr;

    @CalledByNative
    RoomCloseReason(long ptr) {
        this.nativePtr = ptr;
    }

    public @NonNull
    String reason() {
        return nativeReason(nativePtr);
    }

    public boolean isClosedByServer() {
        return nativeIsClosedByServer(nativePtr);
    }

    public boolean roomCloseReason() {
        return nativeRoomCloseReason(nativePtr);
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
    String nativeReason(long self);

    private static native boolean nativeIsClosedByServer(long self);

    private static native boolean nativeRoomCloseReason(long self);

    private static native void nativeFree(long self);
}