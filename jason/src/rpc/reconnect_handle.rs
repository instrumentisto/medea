//! Reconnection for [`RpcSession`].

use std::{rc::Weak, time::Duration};

use backoff::{backoff::Backoff as _, ExponentialBackoff};
use derive_more::{Display, From};
use tracerr::Traced;

use crate::{
    platform,
    rpc::{RpcSession, SessionError},
    utils::JsCaused,
};

/// Errors occurring in a [`ReconnectHandle`].
#[derive(Clone, Debug, From, Display, JsCaused)]
#[js(error = "platform::Error")]
pub enum ReconnectError {
    /// Some [`SessionError`] has occurred while reconnecting.
    #[display(fmt = "{}", _0)]
    Session(#[js(cause)] SessionError),

    /// [`ReconnectHandle`]'s [`Weak`] pointer is detached.
    #[display(fmt = "Reconnector is in detached state")]
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
    /// as `0.0`. This might cause busy loop so its not recommended.
    ///
    /// # Errors
    ///
    /// With [`ReconnectError::Detached`] if [`Weak`] pointer upgrade fails.
    #[allow(clippy::missing_panics_doc)]
    pub async fn reconnect_with_backoff(
        &self,
        starting_delay_ms: u32,
        multiplier: f64,
        max_delay: u32,
        stop_on_max: bool,
    ) -> Result<(), Traced<ReconnectError>> {
        let initial_interval =
            Duration::from_millis(u64::from(starting_delay_ms));
        let mut backoff = ExponentialBackoff {
            current_interval: initial_interval,
            initial_interval,
            randomization_factor: 0.0,
            multiplier,
            max_interval: Duration::from_millis(u64::from(max_delay)),
            start_time: instant::Instant::now(),
            max_elapsed_time: None,
            clock: backoff::SystemClock::default(),
        };

        // match future::select(
        //     states.next(),
        //     Box::pin(timeout),
        // ).await
        loop {
            // Wont panic, since `ExponentialBackoff.max_elapsed_time` is
            // `None`.
            platform::delay_for(backoff.next_backoff().unwrap()).await;
            let inner = self
                .0
                .upgrade()
                .ok_or_else(|| tracerr::new!(ReconnectError::Detached))?;

            if let Err(err) = inner
                .reconnect()
                .await
                .map_err(tracerr::map_from_and_wrap!())
            {
                if stop_on_max
                    && backoff.current_interval == backoff.max_interval
                {
                    break Err(err);
                }
            } else {
                break Ok(());
            }
        }
    }
}

#[cfg(feature = "mockable")]
impl From<Weak<dyn RpcSession>> for ReconnectHandle {
    fn from(session: Weak<dyn RpcSession>) -> Self {
        ReconnectHandle(session)
    }
}
