package com.jason.api;

import android.util.Log;

import androidx.test.filters.SmallTest;

import org.junit.Test;

import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.function.Consumer;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

@SmallTest
public class InstrumentedTest {

    private static final String TAG = "InstrumentedTest";

    @Test
    public void newJason() throws Exception {
        final CountDownLatch done = new CountDownLatch(1);
        ExecutorService executor = Executors.newSingleThreadExecutor();
        Jason jason = new Jason();

        new Thread(() -> {
            try {
                Log.e(TAG, "1 " + Thread.currentThread().getName());
                RoomHandle room = jason.initRoom();
                room.onNewConnection(handle -> {
                    Log.e(TAG, "2 " + Thread.currentThread().getName());

                    try {
                        handle.onRemoteTrackAdded(remoteMediaTrack -> {
                            Log.e(TAG, "3 " + Thread.currentThread().getName());
                            assertTrue(remoteMediaTrack.enabled());

                            remoteMediaTrack.onEnabled(aVoid -> {
                                Log.e(TAG, "4 " + Thread.currentThread().getName());
                                done.countDown();
                            });
                        });
                    } catch (Exception e) {
                        e.printStackTrace();
                    }
                });
                assertEquals(0, jason.mediaManager().enumerateDevices().length);
            } catch (Exception e) {
                e.printStackTrace();
            }
        }).start();

        done.await();
        jason.free();
    }
}
