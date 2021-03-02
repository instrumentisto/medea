package com.jason.app;

import android.app.Application;

import com.jason.api.Jason;

public final class MyApplication extends Application {
    private static final String TAG = "exm MyApplication";
    private static MyApplication app;
    private Jason jason;

    public MyApplication() {
        super();
        app = this;
    }

    public static MyApplication get() {
        return app;
    }

    @Override
    public void onCreate() {
        super.onCreate();
        jason = new Jason();
    }

    public Jason getJason() {
        return jason;
    }
}
