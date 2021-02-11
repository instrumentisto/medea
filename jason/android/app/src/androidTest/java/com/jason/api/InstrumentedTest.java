package com.jason.api;

import android.util.Log;

import androidx.annotation.NonNull;
import androidx.test.filters.SmallTest;

import org.junit.Test;

import static org.junit.Assert.assertEquals;

@SmallTest
public class InstrumentedTest {

    private static final String TAG = "InstrumentedTest";

    @Test
    public void newJason() throws Exception {
        Log.e(TAG, "newJason");
        Log.e(TAG, "1: " + Thread.currentThread().getId() + " " + Thread.currentThread().getName());
        Jason jason = new Jason();
        RoomHandle room = jason.initRoom();
        room.onNewConnection(new ConsumerConnectionHandle() {
            @Override
            public void accept(@NonNull ConnectionHandle handle) {
                Log.e(TAG, "2:" + Thread.currentThread().getId() + " " + Thread.currentThread().getName());
            }
        });
        assertEquals(0, jason.mediaManager().enumerateDevices().length);
        jason.free();
    }
}
