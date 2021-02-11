package com.jason.api;

import androidx.annotation.NonNull;

public final class ConnectionHandle {

    private volatile long nativePtr;

    @CalledByNative
    ConnectionHandle(long ptr) {
        this.nativePtr = ptr;
    }

    public void onClose(@NonNull Callback f) throws Exception {
        nativeOnClose(nativePtr, f);
    }

    public @NonNull
    String getRemoteMemberId() throws Exception {
        return nativeGetRemoteMemberId(nativePtr);
    }

    public void onRemoteTrackAdded(@NonNull ConsumerRemoteMediaTrack f) throws Exception {
        nativeOnRemoteTrackAdded(nativePtr, f);
    }

    public void onQualityScoreUpdate(@NonNull ConsumerShort f) throws Exception {
        nativeOnQualityScoreUpdate(nativePtr, f);
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

    private static native void nativeOnClose(long self, Callback f) throws Exception;

    private static native @NonNull
    String nativeGetRemoteMemberId(long self) throws Exception;

    private static native void nativeOnRemoteTrackAdded(long self, ConsumerRemoteMediaTrack f) throws Exception;

    private static native void nativeOnQualityScoreUpdate(long self, ConsumerShort f) throws Exception;

    private static native void nativeFree(long self);
}