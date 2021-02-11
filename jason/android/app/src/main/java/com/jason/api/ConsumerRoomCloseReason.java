package com.jason.api;

import androidx.annotation.NonNull;

public interface ConsumerRoomCloseReason {

    void accept(@NonNull RoomCloseReason x);

}
