#[no_mangle]
pub extern "C" fn add(i: i64) -> i64 {
    i + 100
}

/// This function should be declared in the header file for the iOS and should
/// pretend to be called.
///
/// This is necessary so that the Swift compiler does not remove the dynamic
/// library from the final application.
#[no_mangle]
pub extern "C" fn dummy_function() {}
