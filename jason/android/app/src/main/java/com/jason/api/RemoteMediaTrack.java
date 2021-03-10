package com.jason.api;

import androidx.annotation.NonNull;

import com.jason.utils.VoidConsumer;

public final class RemoteMediaTrack {

    volatile long nativePtr;

    @CalledByNative
    RemoteMediaTrack(long ptr) {
        this.nativePtr = ptr;
    }

    public boolean enabled() {
        return nativeEnabled(nativePtr);
    }

    public void onEnabled(@NonNull VoidConsumer callback) {
        nativeOnEnabled(nativePtr, callback);
    }

    public void onDisabled(@NonNull VoidConsumer callback) {
        nativeOnDisabled(nativePtr, callback);
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

    private static native boolean nativeEnabled(long self);

    private static native void nativeOnEnabled(long self, VoidConsumer callback);

    private static native void nativeOnDisabled(long self, VoidConsumer callback);

    private static native int nativeKind(long self);

    private static native int nativeMediaSourceKind(long self);

    private static native void nativeFree(long self);
}
