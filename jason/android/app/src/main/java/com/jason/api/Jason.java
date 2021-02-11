package com.jason.api;

import androidx.annotation.NonNull;

public final class Jason {

    static {
        System.loadLibrary("jason_java");
    }

    volatile long nativePtr;

    public Jason() {
        nativePtr = init();
    }

    public @NonNull
    RoomHandle initRoom() {
        return new RoomHandle(nativeInitRoom(nativePtr));
    }

    public @NonNull
    MediaManagerHandle mediaManager() {
        return new MediaManagerHandle(nativeMediaManager(nativePtr));
    }

    @MoveSemantics
    public void closeRoom(@NonNull RoomHandle roomToDelete) {
        long roomToDeletePtr = roomToDelete.nativePtr;
        roomToDelete.nativePtr = 0;

        nativeCloseRoom(nativePtr, roomToDeletePtr);

        ReachabilityFence.reachabilityFence(roomToDelete);
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

    private static native long init();

    private static native long nativeInitRoom(long self);

    private static native long nativeMediaManager(long self);

    private static native void nativeCloseRoom(long self, long roomToDelete);

    private static native void nativeFree(long self);
}
