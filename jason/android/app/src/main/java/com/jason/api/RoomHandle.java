package com.jason.api;

import android.util.Log;

import androidx.annotation.NonNull;
import androidx.annotation.Nullable;

import com.jason.utils.PtrConsumer;

import java.util.function.Consumer;

public final class RoomHandle {

    volatile long nativePtr;

    @CalledByNative
    RoomHandle(long ptr) {
        this.nativePtr = ptr;
    }

    public void join(@NonNull String token, @NonNull AsyncTaskCallback<Void> asyncCb) throws Exception {
        nativeAsyncJoin(nativePtr, token, asyncCb);
    }

    public void onNewConnection(@NonNull Consumer<ConnectionHandle> cb) throws Exception {
        nativeOnNewConnection(nativePtr, (ptr) -> {
            cb.accept(new ConnectionHandle(ptr));
        });
    }

    public void onClose(@NonNull Consumer<RoomCloseReason> cb) throws Exception {
        nativeOnClose(nativePtr, (ptr) -> {
            cb.accept(new RoomCloseReason(ptr));
        });
    }

    public void onLocalTrack(@NonNull Consumer<LocalMediaTrack> cb) throws Exception {
        nativeOnLocalTrack(nativePtr, (ptr) -> cb.accept(new LocalMediaTrack(ptr)));
    }

    public void onFailedLocalMedia(@NonNull Consumer<JasonError> cb) throws Exception {
        nativeOnFailedLocalMedia(nativePtr, (ptr) -> cb.accept(new JasonError(ptr)));
    }

    public void onConnectionLoss(@NonNull Consumer<ReconnectHandle> cb) throws Exception {
        nativeOnConnectionLoss(nativePtr, (ptr) -> cb.accept(new ReconnectHandle(ptr)));
    }

    public void setLocalMediaSettings(@NonNull MediaStreamSettings settings, boolean stopFirst, boolean rollbackOnFail) throws Exception {
        nativeSetLocalMediaSettings(nativePtr, settings.nativePtr, stopFirst, rollbackOnFail);

        ReachabilityFence.reachabilityFence(settings);
    }

    public void muteAudio(@NonNull  AsyncTaskCallback<Void> cb) throws Exception {
        nativeMuteAudio(nativePtr, cb);
    }

    public void unmuteAudio(@NonNull AsyncTaskCallback<Void> cb) throws Exception {
        nativeUnmuteAudio(nativePtr, cb);
    }

    public void muteVideo(@Nullable MediaSourceKind sourceKind, @NonNull AsyncTaskCallback<Void> cb) throws Exception {
        int optionalSourceKind = (sourceKind != null) ? sourceKind.ordinal() : -1;

        nativeMuteVideo(nativePtr, optionalSourceKind, cb);

        ReachabilityFence.reachabilityFence(sourceKind);
    }

    public void unmuteVideo(@Nullable MediaSourceKind sourceKind, @NonNull AsyncTaskCallback<Void> cb) throws Exception {
        int optionalSourceKind = (sourceKind != null) ? sourceKind.ordinal() : -1;

        nativeUnmuteVideo(nativePtr, optionalSourceKind, cb);

        ReachabilityFence.reachabilityFence(sourceKind);
    }

    public void disableAudio(@NonNull AsyncTaskCallback<Void> cb) throws Exception {
        nativeDisableAudio(nativePtr, cb);
    }

    public void enableAudio(@NonNull AsyncTaskCallback<Void> cb) throws Exception {
        nativeEnableAudio(nativePtr, cb);
    }

    public void disableVideo(@Nullable MediaSourceKind sourceKind, @NonNull AsyncTaskCallback<Void> cb) throws Exception {
        int optionalSourceKind = (sourceKind != null) ? sourceKind.ordinal() : -1;

        nativeDisableVideo(nativePtr, optionalSourceKind, cb);

        ReachabilityFence.reachabilityFence(sourceKind);
    }

    public void enableVideo(@Nullable MediaSourceKind sourceKind, @NonNull AsyncTaskCallback<Void> cb) throws Exception {
        int optionalSourceKind = (sourceKind != null) ? sourceKind.ordinal() : -1;

        nativeEnableVideo(nativePtr, optionalSourceKind, cb);

        ReachabilityFence.reachabilityFence(sourceKind);
    }

    public void disableRemoteAudio(@NonNull AsyncTaskCallback<Void> cb) throws Exception {
        nativeDisableRemoteAudio(nativePtr, cb);
    }

    public void disableRemoteVideo(@NonNull AsyncTaskCallback<Void> cb) throws Exception {
        nativeDisableRemoteVideo(nativePtr, cb);
    }

    public void enableRemoteAudio(@NonNull AsyncTaskCallback<Void> cb) throws Exception {
        nativeEnableRemoteAudio(nativePtr, cb);
    }

    public void enableRemoteVideo(@NonNull AsyncTaskCallback<Void> cb) throws Exception {
        nativeEnableRemoteVideo(nativePtr, cb);
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

    private static native void nativeAsyncJoin(long self, String token, AsyncTaskCallback<Void> asyncCb) throws Exception;

    private static native void nativeOnNewConnection(long self, PtrConsumer cb) throws Exception;

    private static native void nativeOnClose(long self, PtrConsumer cb) throws Exception;

    private static native void nativeOnLocalTrack(long self, PtrConsumer cb) throws Exception;

    private static native void nativeOnFailedLocalMedia(long self, PtrConsumer cb) throws Exception;

    private static native void nativeOnConnectionLoss(long self, PtrConsumer cb) throws Exception;

    private static native void nativeSetLocalMediaSettings(long self, long settings, boolean stopFirst, boolean rollbackOnFail) throws Exception;

    private static native void nativeMuteAudio(long self, AsyncTaskCallback<Void> cb) throws Exception;

    private static native void nativeUnmuteAudio(long self, AsyncTaskCallback<Void> cb) throws Exception;

    private static native void nativeMuteVideo(long self, int sourceKind, AsyncTaskCallback<Void> cb) throws Exception;

    private static native void nativeUnmuteVideo(long self, int sourceKind, AsyncTaskCallback<Void> cb) throws Exception;

    private static native void nativeDisableAudio(long self, AsyncTaskCallback<Void> cb) throws Exception;

    private static native void nativeEnableAudio(long self, AsyncTaskCallback<Void> cb) throws Exception;

    private static native void nativeDisableVideo(long self, int sourceKind, AsyncTaskCallback<Void> cb) throws Exception;

    private static native void nativeEnableVideo(long self, int sourceKind, AsyncTaskCallback<Void> cb) throws Exception;

    private static native void nativeDisableRemoteAudio(long self, AsyncTaskCallback<Void> cb) throws Exception;

    private static native void nativeDisableRemoteVideo(long self, AsyncTaskCallback<Void> cb) throws Exception;

    private static native void nativeEnableRemoteAudio(long self, AsyncTaskCallback<Void> cb) throws Exception;

    private static native void nativeEnableRemoteVideo(long self, AsyncTaskCallback<Void> cb) throws Exception;

    private static native void nativeFree(long self);
}
