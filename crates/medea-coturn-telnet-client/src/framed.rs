//! Contains definitions for messages sent to [Coturn] server telnet interface
//! [`CoturnCliRequest`], messages received from Coturn server telnet interface:
//! [`CoturnCliResponse`]. [`CoturnCliCodec`] which encodes and decodes those
//! messages.
//!
//! [Coturn]: https://github.com/coturn/coturn

use std::{
    convert::TryFrom,
    io,
    str::{from_utf8, Utf8Error},
};

use bytes::{BufMut as _, Bytes, BytesMut};
use once_cell::sync::Lazy;
use regex::Regex;
use tokio_util::codec::{Decoder, Encoder};

// Cursor is received when telnet server has finished writing response and is
// ready to receive new requests.
static CURSOR: &str = "> ";

// Received when telnet server awaits for password.
static NEED_PASS: &str = "Enter password: \r\n";

/// Received when telnet server did not recognized last command.
static UNKNOWN_COMMAND: &str = "Unknown command\r\n\r\n";

// Used to check is message can be parsed to CoturnCliResponse::Sessions.
static IS_SESSIONS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"Total sessions: \d"#).unwrap());

// Used to extract session ids from CoturnCliResponse::Sessions.
static EXTRACT_SESSIONS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\d\) id=(.*),").unwrap());

/// Messages that can be received from Coturn telnet server.
#[derive(Clone, Debug, PartialEq)]
pub enum CoturnCliResponse {
    /// Current telnet connection requires authentication. Next message sent to
    /// server should be [`CoturnCliRequest::Auth`].
    EnterPassword,

    /// Coturn server finished processing latest telnet request and is ready to
    /// accept next. You should wait for this message after sending request
    /// to make sure that request succeeded.
    Ready,

    /// Answer to [`CoturnCliRequest::PrintSessions`], contains list of session
    /// ids associated with username provided in
    /// [`CoturnCliRequest::PrintSessions`] message.
    Sessions(Vec<String>),

    /// Coturn telnet server did not recognized last command.
    UnknownCommand,
}

/// Errors that can happen when parsing message received from Coturn via telnet
/// connection.
#[derive(Debug)]
pub enum CoturnResponseParseError {
    /// Could not represent byte slice as `String`.
    BadString(Utf8Error),

    /// Could not determine concrete response type.
    CannotDetermineResponseType(String),

    /// Could not parse provided bytes to determined response type.
    BadResponseFormat(String),
}

impl TryFrom<BytesMut> for CoturnCliResponse {
    type Error = CoturnResponseParseError;

    fn try_from(mut msg: BytesMut) -> Result<Self, Self::Error> {
        // delete cursor if message ends with it
        if msg.ends_with(CURSOR.as_bytes()) {
            msg.truncate(msg.len() - CURSOR.as_bytes().len());
        }

        let msg =
            from_utf8(&msg).map_err(CoturnResponseParseError::BadString)?;

        if msg.is_empty() {
            return Ok(CoturnCliResponse::Ready);
        }

        if msg.ends_with(NEED_PASS) {
            return Ok(CoturnCliResponse::EnterPassword);
        }

        if msg.ends_with(UNKNOWN_COMMAND) {
            return Ok(CoturnCliResponse::UnknownCommand);
        }

        if IS_SESSIONS_REGEX.is_match(msg) {
            let mut session_ids: Vec<String> = Vec::new();
            for mat in EXTRACT_SESSIONS_REGEX.captures_iter(msg) {
                if let Some(id) = mat.get(1) {
                    session_ids.push(id.as_str().to_owned());
                } else {
                    return Err(CoturnResponseParseError::BadResponseFormat(
                        msg.to_owned(),
                    ));
                }
            }
            return Ok(CoturnCliResponse::Sessions(session_ids));
        }

        Err(CoturnResponseParseError::CannotDetermineResponseType(
            msg.to_owned(),
        ))
    }
}

/// Messages that can be sent to Coturn telnet client.
#[derive(Debug)]
pub enum CoturnCliRequest {
    /// Request to authenticate. Contains password. Should be sent when
    /// [`CoturnCliResponse::EnterPassword`] is received.
    Auth(Bytes),

    /// Get Coturn session ids by username.
    PrintSessions(String),

    /// Close Coturn session by its id.
    CloseSession(String),

    /// Ping
    Ping,
}

impl Into<Bytes> for CoturnCliRequest {
    fn into(self) -> Bytes {
        match self {
            CoturnCliRequest::Auth(pass) => pass,
            CoturnCliRequest::PrintSessions(username) => {
                format!("ps {}", username).into()
            }
            CoturnCliRequest::CloseSession(session_id) => {
                format!("cs {}", session_id).into()
            }
            CoturnCliRequest::Ping => "ping".into(),
        }
    }
}

/// Errors that can happen while decoding bytes received to
/// [`CoturnCliResponse`].
#[derive(Debug)]
pub enum CoturnCliCodecError {
    /// Errors that can happen while preforming I/O operations.
    IoError(io::Error),

    /// Errors that can happen when parsing message received from Coturn via
    /// telnet connection.
    CannotParseResponse(CoturnResponseParseError),
}

impl From<io::Error> for CoturnCliCodecError {
    fn from(err: io::Error) -> Self {
        CoturnCliCodecError::IoError(err)
    }
}

impl From<CoturnResponseParseError> for CoturnCliCodecError {
    fn from(err: CoturnResponseParseError) -> Self {
        CoturnCliCodecError::CannotParseResponse(err)
    }
}

/// Adapter that encodes requests and decodes responses received from or sent to
/// [Coturn] server telnet interface.
///
/// [Coturn]: https://github.com/coturn/coturn
#[derive(Copy, Clone, Default, Debug)]
pub struct CoturnCliCodec;

impl Decoder for CoturnCliCodec {
    type Error = CoturnCliCodecError;
    type Item = CoturnCliResponse;

    fn decode(
        &mut self,
        src: &mut BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
        if src.ends_with(CURSOR.as_bytes()) {
            let frame = CoturnCliResponse::try_from(src.split())?;
            Ok(Some(frame))
        } else if src.ends_with(NEED_PASS.as_bytes()) {
            src.clear();
            Ok(Some(CoturnCliResponse::EnterPassword))
        } else {
            Ok(None)
        }
    }
}

impl Encoder for CoturnCliCodec {
    type Error = io::Error;
    type Item = CoturnCliRequest;

    fn encode(
        &mut self,
        item: Self::Item,
        dst: &mut BytesMut,
    ) -> Result<(), Self::Error> {
        let item: Bytes = item.into();
        dst.reserve(item.len());
        dst.put(item);
        Ok(())
    }
}

#[cfg(test)]
mod test {

    use bytes::BytesMut;

    use super::*;

    #[tokio::test]
    async fn parse_greeting() {
        let mut codec = CoturnCliCodec::default();
        let mut greeting: BytesMut = "TURN Server\r\nCoturn-4.5.1.1 'dan \
                                      Eider'\r\n\r\nType '?' for \
                                      help\r\nEnter password: \r\n"
            .into();

        assert_eq!(
            codec.decode(&mut greeting).unwrap().unwrap(),
            CoturnCliResponse::EnterPassword
        );
    }

    #[tokio::test]
    async fn parse_empty_sessions() {
        let mut codec = CoturnCliCodec::default();
        let mut greeting = "\r\n  Total sessions: 0\r\n\r\n> ".into();

        match codec.decode(&mut greeting).unwrap().unwrap() {
            CoturnCliResponse::Sessions(sessions) => {
                assert!(sessions.is_empty());
            }
            _ => unreachable!(),
        }
    }

    #[tokio::test]
    async fn parse_sessions() {
        let mut codec = CoturnCliCodec::default();
        let mut sessions_message = "
    1) id=010000000000000001, user <777_Mireya>:
      realm: medea
      started 545 secs ago
      expiring in 171 secs
      client protocol TCP, relay protocol UDP
      client addr [::1]:56278, server addr [::1]:3478
      relay addr [::1]:58490
      fingerprints enforced: OFF
      mobile: OFF
      usage: rp=878759, rb=704147763, sp=878425, sb=705869096
       rate: r=1299165, s=1302341, total=2601506 (bytes per sec)
      peers:
          ::1
          [::1]:65282

    2) id=001000000000000002, user <777_Mireya>:
      realm: medea
      started 545 secs ago
      expiring in 171 secs
      client protocol UDP, relay protocol UDP
      client addr 192.168.31.183:45096, server addr 127.0.0.1:3478
      relay addr 127.0.0.1:57758
      fingerprints enforced: OFF
      mobile: OFF
      usage: rp=16, rb=1080, sp=15, sb=1568
       rate: r=0, s=0, total=0 (bytes per sec)

    3) id=011000000000000002, user <777_Mireya>:
      realm: medea
      started 545 secs ago
      expiring in 171 secs
      client protocol UDP, relay protocol UDP
      client addr 192.168.31.183:39916, server addr 127.0.0.1:3478
      relay addr 127.0.0.1:55028
      fingerprints enforced: OFF
      mobile: OFF
      usage: rp=17, rb=1212, sp=15, sb=1568
       rate: r=0, s=0, total=0 (bytes per sec)

    4) id=011000000000000003, user <777_Mireya>:
      realm: medea
      started 545 secs ago
      expiring in 171 secs
      client protocol TCP, relay protocol UDP
      client addr [::1]:56276, server addr [::1]:3478
      relay addr [::1]:61957
      fingerprints enforced: OFF
      mobile: OFF
      usage: rp=155, rb=21184, sp=154, sb=23228
       rate: r=0, s=0, total=0 (bytes per sec)
      peers:
          ::1

  Total sessions: 4

> "
        .into();

        match codec.decode(&mut sessions_message).unwrap().unwrap() {
            CoturnCliResponse::Sessions(sessions) => {
                assert_eq!(
                    sessions,
                    vec![
                        "010000000000000001",
                        "001000000000000002",
                        "011000000000000002",
                        "011000000000000003"
                    ]
                );
            }
            _ => unreachable!(),
        }
    }
}
