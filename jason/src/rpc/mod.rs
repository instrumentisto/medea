//! Abstraction over RPC transport.

mod backoff_delayer;
mod heartbeat;
mod reconnect_handle;
mod rpc_session;
pub mod websocket;

use std::rc::Rc;

use async_trait::async_trait;
use derive_more::{Display, From};
use futures::{
    channel::oneshot, future::LocalBoxFuture, stream::LocalBoxStream,
};
use medea_client_api_proto::{
    CloseDescription, CloseReason as CloseByServerReason, Command, Event,
    MemberId, RoomId, Token,
};
use tracerr::Traced;
use url::Url;
use web_sys::CloseEvent;

use crate::utils::{JsCaused, JsError};

#[doc(inline)]
pub use self::{
    backoff_delayer::BackoffDelayer,
    heartbeat::{Heartbeat, HeartbeatError, IdleTimeout, PingInterval},
    reconnect_handle::ReconnectHandle,
    rpc_session::Session,
    websocket::{
        ClientDisconnect, RpcTransport, TransportError, WebSocketRpcClient,
        WebSocketRpcTransport,
    },
};

/// Client to talk with server via Client API RPC.
#[async_trait(?Send)]
#[cfg_attr(feature = "mockable", mockall::automock)]
pub trait RpcSession {
    /// Tries to upgrade [`State`] of this [`RpcClient`] to [`State::Open`].
    ///
    /// This function is also used for reconnection of this [`RpcClient`].
    ///
    /// If [`RpcClient`] is closed than this function will try to establish
    /// new RPC connection.
    ///
    /// If [`RpcClient`] already in [`State::Connecting`] then this function
    /// will not perform one more connection try. It will subsribe to
    /// [`State`] changes and wait for first connection result. And based on
    /// this result - this function will be resolved.
    ///
    /// If [`RpcClient`] already in [`State::Open`] then this function will be
    /// instantly resolved.
    async fn connect(
        self: Rc<Self>,
        url: Url,
        room_id: RoomId,
        member_id: MemberId,
        token: Token,
    ) -> Result<(), Traced<RpcClientError>>;

    async fn reconnect(self: Rc<Self>) -> Result<(), Traced<RpcClientError>>;

    /// Returns [`Stream`] of all [`Event`]s received by this [`RpcClient`].
    ///
    /// [`Stream`]: futures::Stream
    fn subscribe(self: Rc<Self>) -> LocalBoxStream<'static, Event>;

    /// Sends [`Command`] to server.
    fn send_command(&self, command: Command);

    /// [`Future`] which will resolve on normal [`RpcClient`] connection
    /// closing.
    ///
    /// This [`Future`] wouldn't be resolved on abnormal closes. On
    /// abnormal close [`RpcClient::on_connection_loss`] will be thrown.
    ///
    /// [`Future`]: std::future::Future
    fn on_normal_close(
        &self,
    ) -> LocalBoxFuture<'static, Result<CloseReason, oneshot::Canceled>>;

    /// Sets reason, that will be passed to underlying transport when this
    /// client will be dropped.
    fn set_close_reason(&self, close_reason: ClientDisconnect);

    /// Subscribe to connection loss events.
    ///
    /// Connection loss is any unexpected [`RpcTransport`] close. In case of
    /// connection loss, JS side user should select reconnection strategy with
    /// [`ReconnectHandle`] (or simply close [`Room`]).
    ///
    /// [`Room`]: crate::api::Room
    /// [`Stream`]: futures::Stream
    fn on_connection_loss(&self) -> LocalBoxStream<'static, ()>;

    /// Subscribe to reconnected events.
    ///
    /// This will fire when connection to RPC server is reestablished after
    /// connection loss.
    fn on_reconnected(&self) -> LocalBoxStream<'static, ()>;
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

/// The reason of why [`RpcClient`]/[`RpcTransport`] went into
/// [`State::Closed`].
#[derive(Clone, Debug, PartialEq)]
pub enum ClosedStateReason {
    /// Connection with server was lost.
    ConnectionLost(CloseMsg),

    /// Error while creating connection between client and server.
    ConnectionFailed(TransportError),

    /// [`State`] unexpectedly become [`State::Closed`].
    ///
    /// Considered that this [`ClosedStateReason`] will be never provided.
    Unknown,

    /// Indicates that connection with server has never been established.
    NeverConnected,

    /// First received [`ServerMsg`] after [`RpcClient::connect`] is not
    /// [`ServerMsg::RpcSettings`].
    FirstServerMsgIsNotRpcSettings,

    /// Connection has been inactive for a while and thus considered idle
    /// by a client.
    Idle,
}

/// Errors that may occur in [`RpcClient`].
#[derive(Debug, Display, From, JsCaused)]
pub enum RpcClientError {
    /// Occurs if WebSocket connection to remote media server failed.
    #[display(fmt = "Connection failed: {}", _0)]
    RpcTransportError(#[js(cause)] TransportError),

    /// Occurs if the heartbeat cannot be started.
    #[display(fmt = "Start heartbeat failed: {}", _0)]
    CouldNotStartHeartbeat(#[js(cause)] HeartbeatError),

    /// Occurs if `socket` of [`WebSocketRpcClient`] is unexpectedly `None`.
    #[display(fmt = "Socket of 'WebSocketRpcClient' is unexpectedly 'None'.")]
    NoSocket,

    /// Occurs if [`Weak`] pointer to the [`RpcClient`] can't be upgraded to
    /// [`Rc`].
    ///
    /// [`Weak`]: std::rc::Weak
    #[display(fmt = "RpcClient unexpectedly gone.")]
    RpcClientGone,

    /// Occurs if [`RpcClient::connect`] fails.
    #[display(fmt = "Connection failed. {:?}", _0)]
    ConnectionFailed(ClosedStateReason),

    #[display(fmt = "Could not parse URL: {}", _0)]
    UrlParsingError(String),
}

pub fn parse_join_room_url(
    url: &str,
) -> Result<(Url, RoomId, MemberId, Token), Traced<RpcClientError>> {
    static SEGMENTS_PARSE_ERROR_FN: fn() -> Traced<RpcClientError> = || {
        tracerr::new!(RpcClientError::UrlParsingError(
            "URL path must end with 3 segments, e.g.: \
             `/room_id/member_id/token`"
                .to_owned()
        ))
    };

    let mut url = Url::parse(&url).map_err(|err| {
        tracerr::new!(RpcClientError::UrlParsingError(err.to_string()))
    })?;
    url.set_fragment(None);
    url.set_query(None);

    let mut segments = url
        .path_segments()
        .ok_or_else(SEGMENTS_PARSE_ERROR_FN)?
        .rev();
    let token = segments
        .next()
        .ok_or_else(SEGMENTS_PARSE_ERROR_FN)?
        .to_owned()
        .into();
    let member_id = segments
        .next()
        .ok_or_else(SEGMENTS_PARSE_ERROR_FN)?
        .to_owned()
        .into();
    let room_id = segments
        .next()
        .ok_or_else(SEGMENTS_PARSE_ERROR_FN)?
        .to_owned()
        .into();
    url.set_path("/ws");

    Ok((url, room_id, member_id, token))
}

/// Connection with remote was closed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CloseMsg {
    /// Transport was gracefully closed by remote.
    ///
    /// Determines by close code `1000` and existence of
    /// [`CloseByServerReason`].
    Normal(u16, CloseByServerReason),

    /// Connection was unexpectedly closed. Consider reconnecting.
    ///
    /// Unexpected close determines by non-`1000` close code and for close code
    /// `1000` without reason.
    Abnormal(u16),
}

impl From<&CloseEvent> for CloseMsg {
    fn from(event: &CloseEvent) -> Self {
        let code: u16 = event.code();
        match code {
            1000 => {
                if let Ok(description) =
                    serde_json::from_str::<CloseDescription>(&event.reason())
                {
                    Self::Normal(code, description.reason)
                } else {
                    Self::Abnormal(code)
                }
            }
            _ => Self::Abnormal(code),
        }
    }
}
