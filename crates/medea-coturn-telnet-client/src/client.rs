//! Contains [`CoturnTelnetConnection`].

use std::io;

use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio_util::codec::Framed;

use crate::framed::{
    CoturnCliCodec, CoturnCliCodecError, CoturnCliRequest, CoturnCliResponse,
    CoturnResponseParseError,
};

/// Any errors that can be thrown from [`CoturnTelnetConnection`].
#[derive(Debug, derive_more::Display, derive_more::From)]
pub enum CoturnTelnetError {
    /// Underlying transport encountered [`io::Error`]. You should try
    /// recreating [`CoturnTelnetConnection`].
    #[display(fmt = "Underlying transport encountered IoError: {}", _0)]
    IoError(io::Error),
    /// Underlying stream exhausted. You should try recreating
    /// [`CoturnTelnetConnection`].
    #[display(fmt = "Disconnected from Coturn telnet server")]
    Disconnected,
    /// Unable to parse response from Coturn telnet server. This is
    /// unrecoverable error.
    #[display(fmt = "Unable to parse Coturn response: {:?}", _0)]
    MessageParseError(CoturnResponseParseError),
    /// Coturn answered with unexpected message. This is unrecoverable error.
    #[display(fmt = "Unexpected response received {:?}", _0)]
    UnexpectedMessage(CoturnCliResponse),
    /// Authentication failed. This is unrecoverable error.
    #[display(fmt = "Coturn server rejected provided password")]
    WrongPassword,
}

impl From<CoturnCliCodecError> for CoturnTelnetError {
    fn from(err: CoturnCliCodecError) -> Self {
        match err {
            CoturnCliCodecError::IoError(err) => Self::from(err),
            CoturnCliCodecError::CannotParseResponse(err) => Self::from(err),
        }
    }
}

/// Asynchronous connection to remote [Coturn] server telnet interface. You can
/// use this directly, but it is recommended to use this with connection pool
/// from [`crate::pool::Pool`], which takes care of connection lifecycle.
///
/// [Coturn]: https://github.com/coturn/coturn.
#[derive(Debug)]
pub struct CoturnTelnetConnection(Framed<TcpStream, CoturnCliCodec>);

impl CoturnTelnetConnection {
    /// Opens a telnet connection to a remote host using a `TcpStream` and
    /// performs authentication.
    ///
    /// # Errors
    ///
    /// Errors if could not open [`TcpStream`] or authentication failed.
    pub async fn connect<A: ToSocketAddrs, B: Into<Bytes>>(
        addr: A,
        pass: B,
    ) -> Result<CoturnTelnetConnection, CoturnTelnetError> {
        let stream = TcpStream::connect(addr).await?;
        let mut this = Self(Framed::new(stream, CoturnCliCodec::default()));
        this.auth(pass.into()).await?;

        Ok(this)
    }

    /// Returns session ids associated with provided username.
    ///
    /// 1. Sends [`CoturnCliRequest::PrintSessions`] with provided
    /// username.
    /// 2. Awaits for [`CoturnCliResponse::Sessions`].
    ///
    /// # Errors
    ///
    /// Errors if:
    /// 1. Unable to send message to remote.
    /// 2. Transport error while waiting for server response.
    /// 3. Received unexpected (not [`CoturnCliResponse::Sessions`]) response
    /// from remote.
    pub async fn print_sessions(
        &mut self,
        username: String,
    ) -> Result<Vec<String>, CoturnTelnetError> {
        // Send `CoturnCliRequest::PrintSessions`.
        self.0
            .send(CoturnCliRequest::PrintSessions(username))
            .await?;

        // Await for `CoturnCliResponse::Sessions`.
        let response: CoturnCliResponse = self
            .0
            .next()
            .await
            .ok_or_else(|| CoturnTelnetError::Disconnected)??;
        match response {
            CoturnCliResponse::Sessions(sessions) => Ok(sessions),
            _ => Err(CoturnTelnetError::UnexpectedMessage(response)),
        }
    }

    /// Closes session on Coturn server destroying this session allocations and
    /// channels.
    ///
    /// 1. Sends [`CoturnCliRequest::CloseSession`] with specified session id.
    /// 2. Awaits for [`CoturnCliResponse::Ready`].
    ///
    /// # Errors
    ///
    /// Errors if:
    /// 1. Unable to send message to remote.
    /// 2. Transport error while waiting for server response.
    /// 3. Received unexpected (not [`CoturnCliResponse::Ready`]) response from
    /// remote.
    pub async fn delete_session(
        &mut self,
        session_id: String,
    ) -> Result<(), CoturnTelnetError> {
        self.0
            .send(CoturnCliRequest::CloseSession(session_id))
            .await?;

        // Await for `CoturnCliResponse::Ready`.
        let response: CoturnCliResponse = self
            .0
            .next()
            .await
            .ok_or_else(|| CoturnTelnetError::Disconnected)??;
        match response {
            CoturnCliResponse::Ready => Ok(()),
            _ => Err(CoturnTelnetError::UnexpectedMessage(response)),
        }
    }

    /// Closes sessions on Coturn server destroying provided sessions
    /// allocations and channels.
    ///
    /// For each provided session id:
    /// 1. Sends [`CoturnCliRequest::CloseSession`] with specified session id.
    /// 2. Awaits for [`CoturnCliResponse::Ready`].
    ///
    /// # Errors
    ///
    /// Errors if:
    /// 1. Unable to send message to remote.
    /// 2. Transport error while waiting for server response.
    /// 3. Received unexpected (not [`CoturnCliResponse::Ready`]) response from
    /// remote.
    pub async fn delete_sessions<T: IntoIterator<Item = String>>(
        &mut self,
        session_ids: T,
    ) -> Result<(), CoturnTelnetError> {
        for session_id in session_ids {
            self.delete_session(session_id).await?;
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
    /// Errors if:
    /// 1. Transport error while waiting for server response.
    /// 2. First message received is not [`CoturnCliResponse::EnterPassword`].
    /// 3. Unable to send message to remote.
    /// 4. Second message received is not [`CoturnCliResponse::Ready`].
    async fn auth(&mut self, pass: Bytes) -> Result<(), CoturnTelnetError> {
        // Wait for `CoturnCliResponse::EnterPassword`;
        let response = self
            .0
            .next()
            .await
            .ok_or_else(|| CoturnTelnetError::Disconnected)??;

        if let CoturnCliResponse::EnterPassword = response {
        } else {
            return Err(CoturnTelnetError::UnexpectedMessage(response));
        };

        // Send `CoturnCliRequest::Auth` with provided password.
        self.0.send(CoturnCliRequest::Auth(pass)).await?;

        // Wait for `CoturnCliResponse::Ready`.
        let response = self
            .0
            .next()
            .await
            .ok_or_else(|| CoturnTelnetError::Disconnected)??;
        match response {
            CoturnCliResponse::EnterPassword => {
                Err(CoturnTelnetError::WrongPassword)
            }
            CoturnCliResponse::Ready => Ok(()),
            _ => Err(CoturnTelnetError::UnexpectedMessage(response)),
        }
    }

    /// Pings Coturn telnet server.
    ///
    /// # Errors
    ///
    /// Errors if:
    /// 1. Unable to send message to remote.
    /// 2. Transport error while waiting for server response.
    /// 3. First message received is not [`CoturnCliResponse::UnknownCommand`].
    pub async fn ping(&mut self) -> Result<(), CoturnTelnetError> {
        self.0.send(CoturnCliRequest::Ping).await?;

        let response: CoturnCliResponse = self
            .0
            .next()
            .await
            .ok_or_else(|| CoturnTelnetError::Disconnected)??;
        if let CoturnCliResponse::UnknownCommand = response {
        } else {
            return Err(CoturnTelnetError::UnexpectedMessage(response));
        };

        Ok(())
    }
}
