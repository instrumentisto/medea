//! Reconnection for [`RpcSession`].

// TODO: Remove when moving `JasonError` to `api::wasm`.
#![allow(clippy::missing_errors_doc)]

use std::{rc::Weak, time::Duration};

use derive_more::Display;

use crate::{
    platform,
    rpc::{BackoffDelayer, RpcSession},
    utils::{HandlerDetachedError, JasonError, JsCaused},
};

/// Error which indicates that [`RpcSession`]'s (which this [`ReconnectHandle`]
/// tries to reconnect) token is `None`.
#[derive(Debug, Display, JsCaused)]
#[js(error = "platform::Error")]
struct NoTokenError;

/// External handle used to reconnect to a media server when connection is lost.
///
/// This handle will be passed to a `Room.on_connection_loss` callback.
#[derive(Clone)]
pub struct ReconnectHandle(Weak<dyn RpcSession>);

impl ReconnectHandle {
    /// Instantiates new [`ReconnectHandle`] from the given [`RpcSession`]
    /// reference.
    #[inline]
    #[must_use]
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
    ) -> Result<(), JasonError> {
        platform::delay_for(Duration::from_millis(u64::from(delay_ms))).await;

        let rpc = upgrade_or_detached!(self.0, JasonError)?;
        rpc.reconnect().await.map_err(JasonError::from)?;

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
    ) -> Result<(), JasonError> {
        let mut backoff_delayer = BackoffDelayer::new(
            Duration::from_millis(u64::from(starting_delay_ms)),
            multiplier,
            Duration::from_millis(u64::from(max_delay)),
        );
        backoff_delayer.delay().await;
        while upgrade_or_detached!(self.0, JasonError)?
            .reconnect()
            .await
            .is_err()
        {
            backoff_delayer.delay().await;
        }

        Ok(())
    }
}
