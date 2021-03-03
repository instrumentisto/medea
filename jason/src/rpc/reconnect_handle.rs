//! Reconnection for [`RpcSession`].

use std::{rc::Weak, time::Duration};

use tracerr::Traced;
use derive_more::Display;
use derive_more::From;

use crate::{
    platform,
    rpc::{BackoffDelayer, RpcSession},
    utils::{JsCaused},
};
use crate::rpc::SessionError;

/// Errors that may occur in a [`ReconnectHandle`].
#[derive(Clone, From, Display, JsCaused)]
#[js(error = "platform::Error")]
pub enum ReconnectError {
    /// Some [`SessionError`] occurred while reconnecting.
    #[display(fmt = "{}", _0)]
    Session(#[js(cause)] SessionError),

    /// [`ReconnectHandle`]'s [`Weak`] pointer is detached.
    #[display(fmt = "Reconnector is in detached state")]
    Detached,
}

gen_upgrade_macro!(ReconnectError::Detached);

/// External handle that is used to reconnect to the Medea media server on
/// connection loss.
///
/// This handle will be provided into `Room.on_connection_loss` callback.
#[derive(Clone)]
pub struct ReconnectHandle(Weak<dyn RpcSession>);

impl ReconnectHandle {
    /// Instantiates new [`ReconnectHandle`] from the given [`RpcSession`]
    /// reference.
    pub fn new(rpc: Weak<dyn RpcSession>) -> Self {
        Self(rpc)
    }

    /// Tries to reconnect after the provided delay in milliseconds.
    ///
    /// If [`RpcSession`] is already reconnecting then new reconnection attempt
    /// won't be performed. Instead, it will wait for the first reconnection
    /// attempt result and use it here.
    pub async fn reconnect_with_delay(
        &self,
        delay_ms: u32,
    ) -> Result<(), Traced<ReconnectError>> {
        platform::delay_for(Duration::from_millis(u64::from(delay_ms))).await;

        let rpc = upgrade!(self.0)?;
        rpc.reconnect().await.map_err(tracerr::map_from_and_wrap!())?;

        Ok(())
    }

    /// Tries to reconnect [`RpcSession`] in a loop with a growing backoff
    /// delay.
    ///
    /// The first attempt to reconnect is guaranteed to happen no earlier than
    /// `starting_delay_ms`.
    ///
    /// Also, it guarantees that delay between reconnection attempts won't be
    /// greater than `max_delay_ms`.
    ///
    /// After each reconnection attempt, delay between reconnections will be
    /// multiplied by the given `multiplier` until it reaches `max_delay_ms`.
    ///
    /// If [`RpcSession`] is already reconnecting then new reconnection attempt
    /// won't be performed. Instead, it will wait for the first reconnection
    /// attempt result and use it here.
    ///
    /// If `multiplier` is negative number than `multiplier` will be considered
    /// as `0.0`.
    pub async fn reconnect_with_backoff(
        &self,
        starting_delay_ms: u32,
        multiplier: f32,
        max_delay: u32,
    ) -> Result<(), Traced<ReconnectError>> {
        let mut backoff_delayer = BackoffDelayer::new(
            Duration::from_millis(u64::from(starting_delay_ms)),
            multiplier,
            Duration::from_millis(u64::from(max_delay)),
        );
        backoff_delayer.delay().await;
        while upgrade!(self.0)?
            .reconnect()
            .await
            .is_err()
        {
            backoff_delayer.delay().await;
        }

        Ok(())
    }
}
