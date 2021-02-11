package com.jason.api;

import androidx.annotation.NonNull;

public final class ConstraintsUpdateException {

    volatile long nativePtr;

    @CalledByNative
    ConstraintsUpdateException(long ptr) {
        this.nativePtr = ptr;
    }

    public @NonNull
    String name() {
        return nativeName(nativePtr);
    }

    public @NonNull
    java.util.Optional<JasonError> recoverReason() {
        long ret = nativeRecoverReason(nativePtr);
        java.util.Optional<JasonError> convRet;
        if (ret != 0) {
            convRet = java.util.Optional.of(new JasonError(ret));
        } else {
            convRet = java.util.Optional.empty();
        }

        return convRet;
    }

    public @NonNull
    java.util.Optional<JasonError> recoverFailReason() {
        long ret = nativeRecoverFailReason(nativePtr);
        java.util.Optional<JasonError> convRet;
        if (ret != 0) {
            convRet = java.util.Optional.of(new JasonError(ret));
        } else {
            convRet = java.util.Optional.empty();
        }

        return convRet;
    }

    public @NonNull
    java.util.Optional<JasonError> error() {
        long ret = nativeError(nativePtr);
        java.util.Optional<JasonError> convRet;
        if (ret != 0) {
            convRet = java.util.Optional.of(new JasonError(ret));
        } else {
            convRet = java.util.Optional.empty();
        }

        return convRet;
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

    private static native long nativeRecoverReason(long self);

    private static native long nativeRecoverFailReason(long self);

    private static native long nativeError(long self);

    private static native void nativeFree(long self);
}