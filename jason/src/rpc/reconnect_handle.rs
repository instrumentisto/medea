//! Reconnection for [`RpcSession`].

use std::{rc::Weak, time::Duration};

use derive_more::Display;
use tracerr::Traced;

use crate::{
    platform,
    rpc::{BackoffDelayer, RpcSession, SessionError},
    utils::JsCaused,
};

/// Errors occurring in a [`ReconnectHandle`].
#[derive(Clone, Debug, Display, JsCaused)]
#[js(error = "platform::Error")]
pub enum ReconnectError {
    SessionFinished,

    ConnectionLost,

    Internal,

    AuthorizationFailed,

    /// [`ReconnectHandle`]'s [`Weak`] pointer is detached.
    #[display(fmt = "ReconnectHandle is in detached state")]
    Detached,
}

impl From<SessionError> for ReconnectError {
    fn from(_err: SessionError) -> Self {
        // use SessionError as SE;
        // use ReconnectError as RE;
        //
        // match err {
        //     SE::SessionFinished(cr) => RE::SessionFinished(),
        //     SE::NoCredentials => RE::Internal,
        //     SE::AuthorizationFailed => RE::AuthorizationFailed,
        //     SE::RpcClient(client) => match client {
        //         RpcClientError::RpcTransportError(err) => match err {
        //             TransportError::CreateSocket(pe) => {}
        //             TransportError::InitSocket => {}
        //             TransportError::SerializeClientMessage(ser) => {}
        //             TransportError::ParseServerMessage(de) => {}
        //             TransportError::MessageNotString => {}
        //             TransportError::SendMessage(pe) => {}
        //             TransportError::ClosedSocket => {}
        //         },
        //         RpcClientError::ConnectionFailed(err) => {}
        //         RpcClientError::RpcClientGone => RE::Internal,
        //     },
        //     SE::SessionUnexpectedlyDropped => RE::Internal,
        //     SE::ConnectionLost(clr) => RE::ConnectionLost,
        //     SE::NewConnectionInfo => RE::Internal,
        // }
        todo!()
    }
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
    /// as `0.0`.
    ///
    /// # Errors
    ///
    /// With [`ReconnectError::Detached`] if [`Weak`] pointer upgrade fails.
    pub async fn reconnect_with_backoff(
        &self,
        starting_delay_ms: u32,
        multiplier: f64,
        max_delay: u32,
    ) -> Result<(), Traced<ReconnectError>> {
        let mut backoff_delayer = BackoffDelayer::new(
            Duration::from_millis(u64::from(starting_delay_ms)),
            multiplier,
            Duration::from_millis(u64::from(max_delay)),
        );
        backoff_delayer.delay().await;
        while self
            .0
            .upgrade()
            .ok_or_else(|| tracerr::new!(ReconnectError::Detached))?
            .reconnect()
            .await
            .is_err()
        {
            backoff_delayer.delay().await;
        }

        Ok(())
    }
}
