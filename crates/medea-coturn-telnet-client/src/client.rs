//! Asynchronous client to remote [Coturn] server.
//!
//! [Coturn]: https://github.com/coturn/coturn

use std::io;

use bytes::Bytes;
use derive_more::{Display, From};
use futures::{SinkExt, StreamExt};
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio_util::codec::Framed;

use crate::{
    proto::{
        CoturnCliCodec, CoturnCliCodecError, CoturnCliRequest,
        CoturnCliResponse, CoturnResponseParseError,
    },
    sessions_parser::{Session, SessionId},
};

/// Errors that can be returned by [`CoturnTelnetConnection`].
#[derive(Debug, Display, From)]
pub enum CoturnTelnetError {
    /// Underlying transport encountered error on I/O operation.
    ///
    /// You should try to recreate [`CoturnTelnetConnection`].
    #[display(fmt = "Underlying transport failed on I/O operation: {}", _0)]
    IoFailed(io::Error),

    /// Underlying stream exhausted.
    ///
    /// You should try to recreate [`CoturnTelnetConnection`].
    #[display(fmt = "Disconnected from Coturn telnet server")]
    Disconnected,

    /// Unable to parse response from [Coturn] server.
    ///
    /// This is unrecoverable error.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    #[display(fmt = "Unable to parse response: {}", _0)]
    MessageParseError(CoturnResponseParseError),

    /// [Coturn] answered with unexpected message.
    ///
    /// This is unrecoverable error.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    #[display(fmt = "Unexpected response received: {:?}", _0)]
    UnexpectedMessage(CoturnCliResponse),

    /// Authentication failed.
    ///
    /// This is unrecoverable error.
    #[display(fmt = "Coturn server rejected provided password")]
    WrongPassword,
}

impl From<CoturnCliCodecError> for CoturnTelnetError {
    fn from(err: CoturnCliCodecError) -> Self {
        use CoturnCliCodecError::*;
        match err {
            IoFailed(e) => Self::from(e),
            BadResponse(e) => Self::from(e),
        }
    }
}

/// Asynchronous connection to remote [Coturn] server via [Telnet] interface.
///
/// [Coturn]: https://github.com/coturn/coturn
/// [Telnet]: https://en.wikipedia.org/wiki/Telnet
#[derive(Debug)]
pub struct CoturnTelnetConnection(Framed<TcpStream, CoturnCliCodec>);

impl CoturnTelnetConnection {
    /// Opens a [Telnet] connection to a remote host using a [`TcpStream`] and
    /// performs authentication.
    ///
    /// # Errors
    ///
    /// Errors if couldn't open [`TcpStream`] or authentication failed.
    ///
    /// [Telnet]: https://en.wikipedia.org/wiki/Telnet
    pub async fn connect<A: ToSocketAddrs, B: Into<Bytes>>(
        addr: A,
        pass: B,
    ) -> Result<CoturnTelnetConnection, CoturnTelnetError> {
        let stream = TcpStream::connect(addr).await?;
        let mut this = Self(Framed::new(stream, CoturnCliCodec::default()));
        this.auth(pass.into()).await?;
        Ok(this)
    }

    /// Returns session IDs for [Coturn] server associated with the provided
    /// `username`.
    ///
    /// 1. Sends [`CoturnCliRequest::PrintSessions`] with the provided
    ///    `username`.
    /// 2. Awaits for [`CoturnCliResponse::Sessions`].
    ///
    /// # Errors
    ///
    /// - Unable to send message to remote server.
    /// - Transport error while waiting for server response.
    /// - Received an unexpected (not [`CoturnCliResponse::Sessions`]) response
    ///   from remote server.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    pub async fn print_sessions(
        &mut self,
        username: String,
    ) -> Result<Vec<Session>, CoturnTelnetError> {
        use CoturnTelnetError::*;

        self.0
            .send(CoturnCliRequest::PrintSessions(username))
            .await?;

        let response: CoturnCliResponse =
            self.0.next().await.ok_or(Disconnected)??;
        match response {
            CoturnCliResponse::Sessions(sessions) => Ok(sessions),
            _ => Err(UnexpectedMessage(response)),
        }
    }

    /// Closes session on [Coturn] server destroying this session's allocations
    /// and channels.
    ///
    /// 1. Sends [`CoturnCliRequest::CloseSession`] with the provided
    ///    `session_id`.
    /// 2. Awaits for [`CoturnCliResponse::Ready`].
    ///
    /// # Errors
    ///
    /// - Unable to send message to remote server.
    /// - Transport error while waiting for server response.
    /// - Received an unexpected (not [`CoturnCliResponse::Ready`]) response
    ///   from remote server.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    pub async fn delete_session(
        &mut self,
        session_id: SessionId,
    ) -> Result<(), CoturnTelnetError> {
        use CoturnTelnetError::*;

        self.0
            .send(CoturnCliRequest::CloseSession(session_id))
            .await?;

        let response: CoturnCliResponse =
            self.0.next().await.ok_or(Disconnected)??;
        match response {
            CoturnCliResponse::Ready => Ok(()),
            _ => Err(UnexpectedMessage(response)),
        }
    }

    /// Closes multiple sessions on [Coturn] server destroying their allocations
    /// and channels.
    ///
    /// For each provided session id:
    /// 1. Sends [`CoturnCliRequest::CloseSession`] with specified session id.
    /// 2. Awaits for [`CoturnCliResponse::Ready`].
    ///
    /// # Errors
    ///
    /// - Unable to send message to remote server.
    /// - Transport error while waiting for server response.
    /// - Received an unexpected (not [`CoturnCliResponse::Sessions`]) response
    ///   from remote server.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    pub async fn delete_sessions<T: IntoIterator<Item = SessionId>>(
        &mut self,
        session_ids: T,
    ) -> Result<(), CoturnTelnetError> {
        for id in session_ids {
            self.delete_session(id).await?;
        }
        Ok(())
    }

    /// Authenticates [`CoturnTelnetConnection`].
    ///
    /// 1. Awaits for [`CoturnCliResponse::EnterPassword`].
    /// 2. Sends [`CoturnCliRequest::Auth`].
    /// 3. Awaits for [`CoturnCliResponse::Ready`].
    ///
    /// # Errors
    ///
    /// - Unable to send message to remote server.
    /// - Transport error while waiting for server response.
    /// - First message received is not [`CoturnCliResponse::EnterPassword`].
    /// - Second message received is not [`CoturnCliResponse::Ready`].
    async fn auth(&mut self, pass: Bytes) -> Result<(), CoturnTelnetError> {
        use CoturnTelnetError::*;

        let response = self.0.next().await.ok_or(Disconnected)??;
        if let CoturnCliResponse::EnterPassword = response {
        } else {
            return Err(UnexpectedMessage(response));
        };

        self.0.send(CoturnCliRequest::Auth(pass)).await?;

        let response = self.0.next().await.ok_or(Disconnected)??;
        match response {
            CoturnCliResponse::EnterPassword => Err(WrongPassword),
            CoturnCliResponse::Ready => Ok(()),
            _ => Err(UnexpectedMessage(response)),
        }
    }

    /// Pings [Coturn] server via [Telnet].
    ///
    /// # Errors
    ///
    /// - Unable to send message to remote server.
    /// - Transport error while waiting for server response.
    /// - First message received is not [`CoturnCliResponse::UnknownCommand`].
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    /// [Telnet]: https://en.wikipedia.org/wiki/Telnet
    pub async fn ping(&mut self) -> Result<(), CoturnTelnetError> {
        use CoturnTelnetError::*;

        self.0.send(CoturnCliRequest::Ping).await?;

        let response: CoturnCliResponse =
            self.0.next().await.ok_or(Disconnected)??;
        if let CoturnCliResponse::UnknownCommand = response {
            Ok(())
        } else {
            Err(UnexpectedMessage(response))
        }
    }
}
