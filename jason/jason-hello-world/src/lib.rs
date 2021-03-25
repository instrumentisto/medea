#[no_mangle]
pub extern "C" fn add(i: i64) -> i64 {
    i + 100
}

#[no_mangle]
pub extern "C" fn dummy_function() {}
