//! Reconnection for [`RpcSession`].

use std::{rc::Weak, time::Duration};

use derive_more::{Display, From};
use tracerr::Traced;

use crate::{
    platform,
    rpc::{BackoffDelayer, RpcSession, SessionError},
    utils::JsCaused,
};

/// Errors occurring in a [`ReconnectHandle`].
#[derive(Clone, Debug, From, Display, JsCaused)]
#[js(error = "platform::Error")]
pub enum ReconnectError {
    /// Some [`SessionError`] has occurred while reconnecting.
    Session(#[js(cause)] SessionError),

    /// [`ReconnectHandle`]'s [`Weak`] pointer is detached.
    #[display(fmt = "ReconnectHandle is in detached state")]
    Detached,
}

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
    ///
    /// # Errors
    ///
    /// With [`ReconnectError::Detached`] if [`Weak`] pointer upgrade fails.
    ///
    /// With [`ReconnectError::Session`] if error while reconnecting has
    /// occurred.
    pub async fn reconnect_with_delay(
        &self,
        delay_ms: u32,
    ) -> Result<(), Traced<ReconnectError>> {
        platform::delay_for(Duration::from_millis(u64::from(delay_ms))).await;

        let rpc = self
            .0
            .upgrade()
            .ok_or_else(|| tracerr::new!(ReconnectError::Detached))?;

        rpc.reconnect().await.map_err(tracerr::map_from_and_wrap!())
    }

    /// Tries to reconnect [`RpcSession`] in a loop with a growing backoff
    /// delay.
    ///
    /// The first attempt will be performed immediately, and the second attempt
    /// will be performed after `starting_delay_ms`.
    ///
    /// Delay between reconnection attempts won't be greater than
    /// `max_delay_ms`.
    ///
    /// After each reconnection attempt, delay between reconnections will be
    /// multiplied by the given `multiplier` until it reaches `max_delay_ms`.
    ///
    /// If `multiplier` is a negative number then it will be considered as
    /// `0.0`. This might cause a busy loop, so it's not recommended.
    ///
    /// Max elapsed time can be limited with an optional `max_elapsed_time_ms`
    /// argument.
    ///
    /// If [`RpcSession`] is already reconnecting then new reconnection attempt
    /// won't be performed. Instead, it will wait for the first reconnection
    /// attempt result and use it here.
    ///
    /// # Errors
    ///
    /// With [`ReconnectError::Detached`] if [`Weak`] pointer upgrade fails.
    ///
    /// With [`ReconnectError::Session`] if error while reconnecting has
    /// occurred and `max_elapsed_time_ms` is provided.
    pub async fn reconnect_with_backoff(
        &self,
        starting_delay_ms: u32,
        multiplier: f64,
        max_delay: u32,
        max_elapsed_time_ms: Option<u32>,
    ) -> Result<(), Traced<ReconnectError>> {
        BackoffDelayer::new(
            Duration::from_millis(starting_delay_ms.into()),
            multiplier,
            Duration::from_millis(max_delay.into()),
            max_elapsed_time_ms.map(|val| Duration::from_millis(val.into())),
        )
        .retry(|| async {
            self.0
                .upgrade()
                .ok_or_else(|| {
                    backoff::Error::Permanent(tracerr::new!(
                        ReconnectError::Detached
                    ))
                })?
                .reconnect()
                .await
                .map_err(tracerr::map_from_and_wrap!())
                .map_err(backoff::Error::Transient)
        })
        .await
    }
}
