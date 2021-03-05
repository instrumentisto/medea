package com.jason.api;

import androidx.annotation.NonNull;

import java.util.function.Consumer;
import java.util.function.LongConsumer;

public final class ConnectionHandle {

    private volatile long nativePtr;

    @CalledByNative
    ConnectionHandle(long ptr) {
        this.nativePtr = ptr;
    }

    public void onClose(@NonNull Consumer<Void> cb) throws Exception {
        nativeOnClose(nativePtr, addr -> {
            cb.accept(null);
        });
    }

    public @NonNull
    String getRemoteMemberId() throws Exception {
        return nativeGetRemoteMemberId(nativePtr);
    }

    public void onRemoteTrackAdded(@NonNull Consumer<RemoteMediaTrack> cb) throws Exception {
        nativeOnRemoteTrackAdded(nativePtr, addr -> {
            RemoteMediaTrack remoteMediaTrack = new RemoteMediaTrack(addr);
            cb.accept(remoteMediaTrack);
        });
    }

    public void onQualityScoreUpdate(@NonNull Consumer<Short> cb) throws Exception {
        nativeOnQualityScoreUpdate(nativePtr, cb);
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

    private static native void nativeOnClose(long self, LongConsumer cb) throws Exception;

    private static native @NonNull
    String nativeGetRemoteMemberId(long self) throws Exception;

    private static native void nativeOnRemoteTrackAdded(long self, LongConsumer cb) throws Exception;

    private static native void nativeOnQualityScoreUpdate(long self, Consumer<Short> cb) throws Exception;

    private static native void nativeFree(long self);
}
