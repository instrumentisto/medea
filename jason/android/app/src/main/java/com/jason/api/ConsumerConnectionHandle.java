package com.jason.api;

import androidx.annotation.NonNull;

public interface ConsumerConnectionHandle {

    void accept(@NonNull ConnectionHandle x);

}
