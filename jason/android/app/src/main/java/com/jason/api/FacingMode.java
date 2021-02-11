package com.jason.api;


public enum FacingMode {
    User,
    Environment,
    Left,
    Right;

    // TODO: dont throw?
    @CalledByNative
    static FacingMode fromInt(int x) {
        switch (x) {
            case 0:
                return User;
            case 1:
                return Environment;
            case 2:
                return Left;
            case 3:
                return Right;
            default:
                throw new Error("Invalid value for enum FacingMode: " + x);
        }
    }
}
