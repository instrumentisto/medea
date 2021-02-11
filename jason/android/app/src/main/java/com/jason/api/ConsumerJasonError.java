package com.jason.api;

import androidx.annotation.NonNull;

public interface ConsumerJasonError {

    void accept(@NonNull JasonError x);

}
