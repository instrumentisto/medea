package com.jason.api;

import android.app.Application;

public final class MyApplication extends Application {
    private static final String TAG = "exm MyApplication";
    private static MyApplication sSelf;
    private Jason jason;

    public MyApplication() {
        super();
        sSelf = this;
    }

    public static MyApplication get() {
        return sSelf;
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
