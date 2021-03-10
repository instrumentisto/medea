package com.jason.api;

import androidx.annotation.NonNull;

import com.jason.utils.PtrConsumer;
import com.jason.utils.ShortConsumer;
import com.jason.utils.VoidConsumer;

import java.util.function.Consumer;
import java.util.function.LongConsumer;

public final class ConnectionHandle {

    private volatile long nativePtr;

    @CalledByNative
    ConnectionHandle(long ptr) {
        this.nativePtr = ptr;
    }

    public void onClose(@NonNull VoidConsumer cb) throws Exception {
        nativeOnClose(nativePtr, cb);
    }

    public @NonNull
    String getRemoteMemberId() throws Exception {
        return nativeGetRemoteMemberId(nativePtr);
    }

    public void onRemoteTrackAdded(@NonNull Consumer<RemoteMediaTrack> cb) throws Exception {
        nativeOnRemoteTrackAdded(nativePtr, ptr -> cb.accept(new RemoteMediaTrack(ptr)));
    }

    public void onQualityScoreUpdate(@NonNull ShortConsumer cb) throws Exception {
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

    private static native void nativeOnClose(long self, VoidConsumer cb) throws Exception;

    private static native @NonNull
    String nativeGetRemoteMemberId(long self) throws Exception;

    private static native void nativeOnRemoteTrackAdded(long self, PtrConsumer cb) throws Exception;

    private static native void nativeOnQualityScoreUpdate(long self, ShortConsumer cb) throws Exception;

    private static native void nativeFree(long self);
}
