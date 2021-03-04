package com.jason.api;

public interface AsyncTaskCallback<T> {

    @CalledByNative
    void onDone(T t);

    @CalledByNative
    void onError(Throwable e);

}
