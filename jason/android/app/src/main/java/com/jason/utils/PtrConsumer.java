package com.jason.utils;

import java.util.Objects;

@FunctionalInterface
public interface PtrConsumer {
    void accept(long value);

    default PtrConsumer andThen(PtrConsumer after) {
        Objects.requireNonNull(after);
        return (long t) -> { accept(t); after.accept(t); };
    }
}
