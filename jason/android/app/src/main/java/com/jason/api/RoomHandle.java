package com.jason.api;

import androidx.annotation.NonNull;
import androidx.annotation.Nullable;

import java.util.function.Consumer;

public final class RoomHandle {

    volatile long nativePtr;

    @CalledByNative
    RoomHandle(long ptr) {
        this.nativePtr = ptr;
    }

    public void join(@NonNull String token) throws Exception {
        nativeJoin(nativePtr, token);
    }

    public void onNewConnection(@NonNull Consumer<ConnectionHandle> cb) throws Exception {
        nativeOnNewConnection(nativePtr, cb);
    }

    public void onClose(@NonNull Consumer<RoomCloseReason> cb) throws Exception {
        nativeOnClose(nativePtr, cb);
    }

    public void onLocalTrack(@NonNull Consumer<LocalMediaTrack> cb) throws Exception {
        nativeOnLocalTrack(nativePtr, cb);
    }

    public void onFailedLocalMedia(@NonNull Consumer<JasonError> cb) throws Exception {
        nativeOnFailedLocalMedia(nativePtr, cb);
    }

    public void onConnectionLoss(@NonNull Consumer<ReconnectHandle> cb) throws Exception {
        nativeOnConnectionLoss(nativePtr, cb);
    }

    public void setLocalMediaSettings(@NonNull MediaStreamSettings settings, boolean stopFirst, boolean rollbackOnFail) throws Exception {
        nativeSetLocalMediaSettings(nativePtr, settings.nativePtr, stopFirst, rollbackOnFail);

        ReachabilityFence.reachabilityFence(settings);
    }

    public void muteAudio() throws Exception {
        nativeMuteAudio(nativePtr);
    }

    public void unmuteAudio() throws Exception {
        nativeUnmuteAudio(nativePtr);
    }

    public void muteVideo(@Nullable MediaSourceKind sourceKind) throws Exception {
        int optionalSourceKind = (sourceKind != null) ? sourceKind.ordinal() : -1;

        nativeMuteVideo(nativePtr, optionalSourceKind);

        ReachabilityFence.reachabilityFence(sourceKind);
    }

    public void unmuteVideo(@Nullable MediaSourceKind sourceKind) throws Exception {
        int optionalSourceKind = (sourceKind != null) ? sourceKind.ordinal() : -1;

        nativeUnmuteVideo(nativePtr, optionalSourceKind);

        ReachabilityFence.reachabilityFence(sourceKind);
    }

    public void disableAudio() throws Exception {
        nativeDisableAudio(nativePtr);
    }

    public void enableAudio() throws Exception {
        nativeEnableAudio(nativePtr);
    }

    public void disableVideo(@Nullable MediaSourceKind sourceKind) throws Exception {
        int optionalSourceKind = (sourceKind != null) ? sourceKind.ordinal() : -1;

        nativeDisableVideo(nativePtr, optionalSourceKind);

        ReachabilityFence.reachabilityFence(sourceKind);
    }

    public void enableVideo(@Nullable MediaSourceKind sourceKind) throws Exception {
        int optionalSourceKind = (sourceKind != null) ? sourceKind.ordinal() : -1;

        nativeEnableVideo(nativePtr, optionalSourceKind);

        ReachabilityFence.reachabilityFence(sourceKind);
    }

    public void disableRemoteAudio() throws Exception {
        nativeDisableRemoteAudio(nativePtr);
    }

    public void disableRemoteVideo() throws Exception {
        nativeDisableRemoteVideo(nativePtr);
    }

    public void enableRemoteAudio() throws Exception {
        nativeEnableRemoteAudio(nativePtr);
    }

    public void enableRemoteVideo() throws Exception {
        nativeEnableRemoteVideo(nativePtr);
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

    private static native void nativeJoin(long self, @NonNull String token) throws Exception;

    private static native void nativeOnNewConnection(long self, Consumer<ConnectionHandle> cb) throws Exception;

    private static native void nativeOnClose(long self, Consumer<RoomCloseReason> cb) throws Exception;

    private static native void nativeOnLocalTrack(long self, Consumer<LocalMediaTrack> cb) throws Exception;

    private static native void nativeOnFailedLocalMedia(long self, Consumer<JasonError> cb) throws Exception;

    private static native void nativeOnConnectionLoss(long self, Consumer<ReconnectHandle> cb) throws Exception;

    private static native void nativeSetLocalMediaSettings(long self, long settings, boolean stopFirst, boolean rollbackOnFail) throws Exception;

    private static native void nativeMuteAudio(long self) throws Exception;

    private static native void nativeUnmuteAudio(long self) throws Exception;

    private static native void nativeMuteVideo(long self, int sourceKind) throws Exception;

    private static native void nativeUnmuteVideo(long self, int sourceKind) throws Exception;

    private static native void nativeDisableAudio(long self) throws Exception;

    private static native void nativeEnableAudio(long self) throws Exception;

    private static native void nativeDisableVideo(long self, int sourceKind) throws Exception;

    private static native void nativeEnableVideo(long self, int sourceKind) throws Exception;

    private static native void nativeDisableRemoteAudio(long self) throws Exception;

    private static native void nativeDisableRemoteVideo(long self) throws Exception;

    private static native void nativeEnableRemoteAudio(long self) throws Exception;

    private static native void nativeEnableRemoteVideo(long self) throws Exception;

    private static native void nativeFree(long self);
}