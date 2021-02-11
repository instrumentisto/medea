package com.jason.api;

// TODO: merge with JasonError
public class JasonException extends RuntimeException {

    private final String name;
    private final String trace;

    public JasonException(String name, String msg, String trace) {
        super(msg);
        this.name = name;
        this.trace = trace;
    }

    public JasonException(String name, String msg, String trace, Throwable cause) {
        super(msg, cause);
        this.name = name;
        this.trace = trace;
    }
}