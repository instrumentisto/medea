use super::ForeignClass;

#[cfg(feature = "mockable")]
pub use self::mock::ReconnectHandle;
#[cfg(not(feature = "mockable"))]
pub use crate::rpc::ReconnectHandle;

impl ForeignClass for ReconnectHandle {}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__free(this: *mut ReconnectHandle) {
    ReconnectHandle::from_ptr(this);
}

#[cfg(feature = "mockable")]
mod mock {
    pub struct ReconnectHandle;

    impl ReconnectHandle {
        // pub async fn reconnect_with_delay(&self, delay_ms: u32) -> Result<(),
        // JasonError> pub async fn reconnect_with_backoff(&self,
        // starting_delay_ms: u32, multiplier: f32,  max_delay: u32) ->
        // Result<(), JasonError>
    }
}
