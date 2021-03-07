package com.jason.api;

import android.util.Log;

import androidx.test.filters.SmallTest;

import com.google.common.util.concurrent.ListenableFuture;
import com.google.common.util.concurrent.ListenableFutureTask;

import org.junit.Test;

import java.util.concurrent.Callable;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.FutureTask;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotEquals;
import static org.junit.Assert.assertTrue;

@SmallTest
public class InstrumentedTest {

//    private static final String TAG = "InstrumentedTest";

    @Test
    public void testCallbacks() throws Exception {
        final CountDownLatch done = new CountDownLatch(1);
        Jason jason = new Jason();

        AtomicLong callerThreadId = new AtomicLong();
        AtomicLong callback1ThreadId = new AtomicLong();
        AtomicLong callback2ThreadId = new AtomicLong();
        AtomicLong callback3ThreadId = new AtomicLong();

        new Thread(() -> {
            callerThreadId.set(Thread.currentThread().getId());
            try {
                RoomHandle room = jason.initRoom();
                Log.d("a", "1");
                room.onNewConnection(handle -> {
                    done.countDown();
//                    Log.d("a", "2");
//                    callback1ThreadId.set(Thread.currentThread().getId());
//
//                    try {
//                        handle.onRemoteTrackAdded(remoteMediaTrack -> {
//                            Log.d("a", "3");
//                            callback2ThreadId.set(Thread.currentThread().getId());
//                            assertTrue(remoteMediaTrack.enabled());
//
//                            remoteMediaTrack.onEnabled(aVoid -> {
//                                callback3ThreadId.set(Thread.currentThread().getId());
//                                done.countDown();
//                            });
//                        });
//                    } catch (Exception e) {
//                        e.printStackTrace();
//                    }
                });
                assertEquals(0, jason.mediaManager().enumerateDevices().length);
            } catch (Exception e) {
                e.printStackTrace();
            }
        }).start();

        done.await();
        jason.free();

        assertEquals(callback1ThreadId.longValue(), callback2ThreadId.longValue());
        assertEquals(callback2ThreadId.longValue(), callback3ThreadId.longValue());
        assertNotEquals(callback1ThreadId.longValue(), callerThreadId.longValue());
    }

    @Test
    public void testAsync() throws Exception {
        Jason jason = new Jason();
        RoomHandle handle = jason.initRoom();
        CountDownLatch latch = new CountDownLatch(1);
        handle.join("this is token", new AsyncTaskCallback<Void>() {
            @Override
            public void onDone(Void nothing) {
                latch.countDown();
            }

            @Override
            public void onError(Throwable e) {

            }
        });

        latch.await();
    }
}
