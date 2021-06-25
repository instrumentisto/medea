//! Abstraction over RPC transport.

mod backoff_delayer;
mod heartbeat;
mod reconnect_handle;
mod rpc_session;
pub mod websocket;

use std::str::FromStr;

use derive_more::{AsRef, Display, From};
use medea_client_api_proto::{
    CloseReason as CloseByServerReason, Credential, MemberId, RoomId,
};
use tracerr::Traced;
use url::Url;

use crate::{platform, utils::JsCaused};

#[cfg(feature = "mockable")]
pub use self::rpc_session::MockRpcSession;
#[doc(inline)]
pub use self::{
    backoff_delayer::BackoffDelayer,
    heartbeat::{Heartbeat, IdleTimeout, PingInterval},
    reconnect_handle::{ReconnectError, ReconnectHandle},
    rpc_session::{
        RpcSession, SessionError, SessionState, WebSocketRpcSession,
    },
    websocket::{ClientDisconnect, RpcEvent, WebSocketRpcClient},
};

/// [`Url`] to which transport layer will connect.
#[derive(AsRef, Clone, Debug, Eq, From, PartialEq)]
#[as_ref(forward)]
pub struct ApiUrl(Url);

/// Information about [`RpcSession`] connection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionInfo {
    /// [`Url`] to which transport layer will connect.
    url: ApiUrl,

    /// [`RoomId`] of the `Room` for which [`RpcSession`] is created.
    room_id: RoomId,

    /// [`MemberId`] of the `Member` for which [`RpcSession`] is created.
    member_id: MemberId,

    /// [`Credential`] for connecting [`RpcSession`].
    credential: Credential,
}

impl ConnectionInfo {
    /// Returns [`ApiUrl`] to which transport layer will connect.
    #[inline]
    #[must_use]
    pub fn url(&self) -> &ApiUrl {
        &self.url
    }

    /// Returns [`RoomId`] of the `Room` for which [`RpcSession`] is created.
    #[inline]
    #[must_use]
    pub fn room_id(&self) -> &RoomId {
        &self.room_id
    }

    /// Returns [`MemberId`] of the `Member` for which [`RpcSession`] is
    /// created.
    #[inline]
    #[must_use]
    pub fn member_id(&self) -> &MemberId {
        &self.member_id
    }

    /// Returns [`Credential`] for connecting [`RpcSession`].
    #[inline]
    #[must_use]
    pub fn credential(&self) -> &Credential {
        &self.credential
    }
}

/// Errors which can occur while [`ConnectionInfo`] parsing from the [`str`].
#[derive(Clone, Debug, Display, JsCaused)]
#[js(error = "platform::Error")]
pub enum ConnectionInfoParseError {
    /// [`Url::parse`] returned error.
    #[display(fmt = "Failed to parse provided URL: {}", _0)]
    UrlParse(url::ParseError),

    /// Provided URL doesn't have important segments.
    #[display(fmt = "Provided URL doesn't have important segments")]
    NotEnoughSegments,

    /// Provided URL doesn't contain auth token.
    #[display(fmt = "Provided URL does not contain auth token")]
    NoToken,
}

impl FromStr for ConnectionInfo {
    type Err = Traced<ConnectionInfoParseError>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use ConnectionInfoParseError as E;

        let mut url =
            Url::parse(s).map_err(|err| tracerr::new!(E::UrlParse(err)))?;

        let credential = url
            .query_pairs()
            .find(|(key, _)| key.as_ref() == "token")
            .ok_or_else(|| tracerr::new!(E::NoToken))?
            .1
            .clone()
            .into();

        url.set_fragment(None);
        url.set_query(None);

        let mut segments = url
            .path_segments()
            .ok_or_else(|| tracerr::new!(E::NotEnoughSegments))?
            .rev();
        let member_id = segments
            .next()
            .ok_or_else(|| tracerr::new!(E::NotEnoughSegments))?
            .to_owned()
            .into();
        let room_id = segments
            .next()
            .ok_or_else(|| tracerr::new!(E::NotEnoughSegments))?
            .to_owned()
            .into();

        // Remove last two segments. Safe to unwrap cause we already made all
        // necessary checks.
        url.path_segments_mut().unwrap().pop().pop();

        Ok(ConnectionInfo {
            url: url.into(),
            room_id,
            member_id,
            credential,
        })
    }
}

/// Reasons of closing by client side and server side.
#[derive(Copy, Clone, Display, Debug, Eq, PartialEq)]
pub enum CloseReason {
    /// Closed by server.
    ByServer(CloseByServerReason),

    /// Closed by client.
    #[display(fmt = "{}", reason)]
    ByClient {
        /// Reason of closing.
        reason: ClientDisconnect,

        /// Is closing considered as error.
        is_err: bool,
    },
}

/// The reason of why [`WebSocketRpcClient`]/[`platform::RpcTransport`] went
/// into a closed state.
#[derive(Clone, Debug, PartialEq)]
pub enum ClosedStateReason {
    /// Indicates that connection with server has never been established.
    NeverConnected,

    /// Failed to establish a connection between a client and a server.
    CouldNotEstablish(platform::TransportError),

    /// Lost a connection with a server.
    ConnectionLost(ConnectionLostReason),

    /// First received [`ServerMsg`] after [`WebSocketRpcClient::connect`] is
    /// not [`ServerMsg::RpcSettings`][1].
    ///
    /// [`ServerMsg`]: medea_client_api_proto::ServerMsg
    /// [1]: medea_client_api_proto::ServerMsg::RpcSettings
    FirstServerMsgIsNotRpcSettings,
}

/// Reason of why [`WebSocketRpcClient`]/[`platform::RpcTransport`] lost
/// connection with a server.
#[derive(Clone, Copy, Debug, Display, PartialEq)]
pub enum ConnectionLostReason {
    /// Connection has been closed with a close frame and the provided message.
    WithMessage(CloseMsg),

    /// Connection has been inactive for a while and thus considered idle
    /// by a client.
    Idle,
}

/// Errors that may occur in [`WebSocketRpcClient`].
#[derive(Clone, Debug, Display, From, JsCaused)]
#[js(error = "platform::Error")]
pub enum RpcClientError {
    /// Occurs if WebSocket connection to remote media server failed.
    #[display(fmt = "Connection failed: {}", _0)]
    RpcTransportError(#[js(cause)] platform::TransportError),

    /// Occurs if [`Weak`] pointer to the [`WebSocketRpcClient`] can't be
    /// upgraded to [`Rc`].
    ///
    /// [`Rc`]: std::rc::Rc
    /// [`Weak`]: std::rc::Weak
    #[display(fmt = "RpcClient unexpectedly gone.")]
    RpcClientGone,

    /// Occurs if [`WebSocketRpcClient::connect`] fails.
    #[display(fmt = "Connection failed. {:?}", _0)]
    ConnectionFailed(ClosedStateReason),
}

/// Connection with remote was closed.
#[derive(Clone, Copy, Debug, Display, PartialEq)]
pub enum CloseMsg {
    /// Transport was gracefully closed by remote.
    ///
    /// Determines by close code `1000` and existence of
    /// [`CloseByServerReason`].
    #[display(fmt = "Normal. Code: {}, Reason: {}", _0, _1)]
    Normal(u16, CloseByServerReason),

    /// Connection was unexpectedly closed. Consider reconnecting.
    ///
    /// Unexpected close determines by non-`1000` close code and for close code
    /// `1000` without reason.
    #[display(fmt = "Abnormal. Code: {}", _0)]
    Abnormal(u16),
}
