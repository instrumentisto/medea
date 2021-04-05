pub struct ReconnectHandle;

// TODO: all methods
#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__reconnect_with_delay(
    this: *mut ReconnectHandle,
    delay_ms: u32,
) {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__free(
    this: *mut ReconnectHandle,
) {
    Box::from_raw(this);
}
