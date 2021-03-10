package com.jason.api;

import androidx.annotation.NonNull;

// TODO: throwable / runtime exception
public final class JasonError {

    volatile long nativePtr;

    // TODO: just pass strings, we dont really need native object here - its a dto
    @CalledByNative
    JasonError(long ptr) {
        this.nativePtr = ptr;
    }

    public @NonNull
    String name() {
        return nativeName(nativePtr);
    }

    public @NonNull
    String message() {
        return nativeMessage(nativePtr);
    }

    public @NonNull
    String trace() {
        return nativeTrace(nativePtr);
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
    String nativeName(long self);

    private static native @NonNull
    String nativeMessage(long self);

    private static native @NonNull
    String nativeTrace(long self);

    private static native void nativeFree(long self);
}
