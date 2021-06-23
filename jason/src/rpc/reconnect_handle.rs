//! Reconnection for [`RpcSession`].

use std::{borrow::Cow, rc::Weak, time::Duration};

use derive_more::Display;
use tracerr::Traced;

use crate::{
    platform,
    rpc::{
        rpc_session::ConnectionLostReason, BackoffDelayer, CloseReason,
        RpcSession, SessionError,
    },
    utils::JsCaused,
};

/// Errors occurring in a [`ReconnectHandle`].
#[derive(Clone, Debug, Display, JsCaused)]
#[js(error = "platform::Error")]
pub enum ReconnectError {
    /// Connection with a server was lost.
    ///
    /// This usually means that some transport error occurred, so a client can
    /// continue performing reconnecting attempts.
    #[display(fmt = "Connection with a server was lost: {}", _0)]
    ConnectionLost(ConnectionLostReason),

    /// Could not authorize an RPC session.
    ///
    /// This usually means that authentication data a client provides is
    /// obsolete.
    #[display(fmt = "Failed to authorize RPC session")]
    AuthorizationFailed,

    /// RPC session has benn finished. This is a terminal state.
    #[display(fmt = "RPC session finished with {:?} close reason", _0)]
    SessionFinished(CloseReason),

    /// Internal error that is not meant to be handled by external users.
    ///
    /// This is a programmatic error.
    Internal(Cow<'static, str>),

    /// [`ReconnectHandle`]'s [`Weak`] pointer is detached.
    #[display(fmt = "ReconnectHandle is in detached state")]
    Detached,
}

impl From<SessionError> for ReconnectError {
    #[inline]
    fn from(err: SessionError) -> Self {
        use SessionError as SE;
        match err {
            SE::SessionFinished(cr) => Self::SessionFinished(cr),
            SE::NoCredentials
            | SE::SessionUnexpectedlyDropped
            | SE::NewConnectionInfo => Self::Internal(err.to_string().into()),
            SE::AuthorizationFailed => Self::AuthorizationFailed,
            SE::RpcClient(client) => Self::Internal(client.to_string().into()),
            SE::ConnectionLost(clr) => Self::ConnectionLost(clr),
        }
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
    /// See [`ReconnectError`] for details.
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
    /// See [`ReconnectError`] for details.
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
