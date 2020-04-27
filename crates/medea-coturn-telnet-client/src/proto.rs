//! [Telnet] messages to operate with [Coturn] and their encoding.
//!
//! [Coturn]: https://github.com/coturn/coturn
//! [Telnet]: https://en.wikipedia.org/wiki/Telnet

use std::{
    convert::TryFrom,
    io,
    str::{from_utf8, Utf8Error},
};

use bytes::{BufMut as _, Bytes, BytesMut};
use derive_more::{Display, From};
use once_cell::sync::Lazy;
use regex::Regex;
use tokio_util::codec::{Decoder, Encoder};

/// [`CURSOR`] is received whenever [Telnet] server has finished writing
/// response and is ready to receive new requests.
///
/// [Telnet]: https://en.wikipedia.org/wiki/Telnet
static CURSOR: &str = "> ";

/// Received whenever [Telnet] server awaits for password.
///
/// [Telnet]: https://en.wikipedia.org/wiki/Telnet
static NEED_PASS: &str = "Enter password: \r\n";

/// Received whenever [Telnet] server didn't recognized last command.
///
/// [Telnet]: https://en.wikipedia.org/wiki/Telnet
static UNKNOWN_COMMAND: &str = "Unknown command\r\n\r\n";

/// Regular expression to check if message can be parsed as
/// [`CoturnCliResponse::Sessions`].
static IS_SESSIONS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"Total sessions: \d"#).unwrap());

/// Regular expression to extract session IDs from
/// [`CoturnCliResponse::Sessions`].
static EXTRACT_SESSIONS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\d\) id=(.*),").unwrap());

/// Message that is received from [Coturn] server via [Telnet].
///
/// [Coturn]: https://github.com/coturn/coturn
/// [Telnet]: https://en.wikipedia.org/wiki/Telnet
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoturnCliResponse {
    /// Current [Telnet] connection requires authentication.
    ///
    /// Next message sent to server should be [`CoturnCliRequest::Auth`].
    ///
    /// [Telnet]: https://en.wikipedia.org/wiki/Telnet
    EnterPassword,

    /// [Coturn] server has finished processing latest [Telnet] request and
    /// is ready to accept the next one.
    ///
    /// You should wait for this message after sending request to make sure
    /// that the request has succeeded.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    /// [Telnet]: https://en.wikipedia.org/wiki/Telnet
    Ready,

    /// Answer to [`CoturnCliRequest::PrintSessions`], which contains list of
    /// session IDs associated with the provided username in
    /// [`CoturnCliRequest::PrintSessions`] message.
    Sessions(Vec<String>),

    /// [Coturn] server hasn't recognized last [Telnet] command.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    /// [Telnet]: https://en.wikipedia.org/wiki/Telnet
    UnknownCommand,
}

/// Errors that can happen when parsing message received from [Coturn] via
/// [Telnet] connection.
///
/// [Coturn]: https://github.com/coturn/coturn
/// [Telnet]: https://en.wikipedia.org/wiki/Telnet
#[derive(Debug, Display, From)] // TODO: derive(Error) with derive_more
pub enum CoturnResponseParseError {
    /// Couldn't parse provided bytes to determined response type.
    #[display(fmt = "Bad response format: {}", _0)]
    #[from(ignore)]
    BadResponseFormat(String),

    /// Failed to determine concrete response type.
    #[display(fmt = "Bad response type: {}", _0)]
    #[from(ignore)]
    BadResponseType(String),

    /// Failed to represent provided bytes as [`String`].
    #[display(fmt = "Cannot convert to String: {}", _0)]
    NonUtf8String(Utf8Error),
}

impl TryFrom<BytesMut> for CoturnCliResponse {
    type Error = CoturnResponseParseError;

    fn try_from(mut msg: BytesMut) -> Result<Self, Self::Error> {
        use CoturnResponseParseError::{BadResponseFormat, BadResponseType};

        // delete cursor if message ends with it
        if msg.ends_with(CURSOR.as_bytes()) {
            msg.truncate(msg.len() - CURSOR.as_bytes().len());
        }

        let msg = from_utf8(&msg)?;

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
            let mut ids: Vec<String> = Vec::new();
            for mat in EXTRACT_SESSIONS_REGEX.captures_iter(msg) {
                if let Some(id) = mat.get(1) {
                    ids.push(id.as_str().to_owned());
                } else {
                    return Err(BadResponseFormat(msg.to_owned()));
                }
            }
            return Ok(CoturnCliResponse::Sessions(ids));
        }

        Err(BadResponseType(msg.to_owned()))
    }
}

/// Messages that can be sent to [Coturn] server via [Telnet].
///
/// [Coturn]: https://github.com/coturn/coturn
/// [Telnet]: https://en.wikipedia.org/wiki/Telnet
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoturnCliRequest {
    /// Authentication request. Contains password. Should be sent when
    /// [`CoturnCliResponse::EnterPassword`] is received.
    Auth(Bytes),

    /// Request to retrieve [Coturn] session IDs by username.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    PrintSessions(String),

    /// Close [Coturn] session by its ID.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    CloseSession(String),

    /// Ping request.
    Ping,
}

impl From<CoturnCliRequest> for Bytes {
    fn from(req: CoturnCliRequest) -> Self {
        use CoturnCliRequest::{Auth, CloseSession, Ping, PrintSessions};
        match req {
            Auth(pass) => pass,
            PrintSessions(username) => format!("ps {}", username).into(),
            CloseSession(session_id) => format!("cs {}", session_id).into(),
            Ping => "ping".into(),
        }
    }
}

/// Errors that can happen while decoding bytes received as
/// [`CoturnCliResponse`].
#[derive(Debug, Display, From)] // TODO: derive(Error) with derive_more
pub enum CoturnCliCodecError {
    /// Failed to perform I/O operation.
    #[display(fmt = "I/O operation failed: {}", _0)]
    IoFailed(io::Error),

    /// Failed to parse received response from [Coturn].
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    #[display(fmt = "Cannot parse response: {}", _0)]
    BadResponse(CoturnResponseParseError),
}

/// Adapter for encoding [`CoturnCliRequest`]s and decoding
/// [`CoturnCliResponse`]s received from or sent to
/// [Coturn] server via [Telnet] interface.
///
/// [Coturn]: https://github.com/coturn/coturn
/// [Telnet]: https://en.wikipedia.org/wiki/Telnet
#[derive(Clone, Copy, Debug, Default)]
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

impl Encoder<CoturnCliRequest> for CoturnCliCodec {
    type Error = io::Error;

    fn encode(
        &mut self,
        req: CoturnCliRequest,
        dst: &mut BytesMut,
    ) -> Result<(), Self::Error> {
        let req: Bytes = req.into();
        dst.reserve(req.len());
        dst.put(req);
        Ok(())
    }
}

#[cfg(test)]
mod spec {
    use super::*;

    #[tokio::test]
    async fn parses_greeting() {
        let mut codec = CoturnCliCodec::default();
        #[rustfmt::skip]
        let mut greeting = "\
        TURN Server\r\n\
        Coturn-4.5.1.1 'dan Eider'\r\n\
        \r\n\
        Type '?' for help\r\n\
        Enter password: \r\n"
            .into();

        let decoded = codec
            .decode(&mut greeting)
            .expect("Failed to decode")
            .unwrap();
        assert_eq!(decoded, CoturnCliResponse::EnterPassword);
    }

    #[tokio::test]
    async fn parses_empty_sessions() {
        let mut codec = CoturnCliCodec::default();
        let mut greeting = "\r\n  Total sessions: 0\r\n\r\n> ".into();

        match codec
            .decode(&mut greeting)
            .expect("Failed to decode")
            .unwrap()
        {
            CoturnCliResponse::Sessions(sessions) => {
                assert!(sessions.is_empty());
            }
            _ => unreachable!(),
        }
    }

    #[tokio::test]
    async fn parses_sessions() {
        let mut codec = CoturnCliCodec::default();
        let mut message = "
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

        match codec
            .decode(&mut message)
            .expect("Failed to decode")
            .unwrap()
        {
            CoturnCliResponse::Sessions(sessions) => {
                assert_eq!(
                    sessions,
                    vec![
                        "010000000000000001",
                        "001000000000000002",
                        "011000000000000002",
                        "011000000000000003",
                    ],
                );
            }
            _ => unreachable!(),
        }
    }
}
