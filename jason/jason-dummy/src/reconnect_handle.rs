pub struct ReconnectHandle;

impl ReconnectHandle {
    // pub async fn reconnect_with_delay(&self, delay_ms: u32) -> Result<(),
    // JasonError> pub async fn reconnect_with_backoff(&self,
    // starting_delay_ms: u32, multiplier: f32,  max_delay: u32) -> Result<(),
    // JasonError>
}

#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__free(this: *mut ReconnectHandle) {
    Box::from_raw(this);
}
