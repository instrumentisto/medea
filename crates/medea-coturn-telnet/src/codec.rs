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
use regex::Regex;
use tokio_util::codec::{Decoder, Encoder};

use crate::sessions_parser::{parse_sessions, Session};

// Cursor is received when telnet server has finished writing response and is
// ready to receive new requests.
static CURSOR: &str = "> ";

// Received when telnet server awaits for password.
static NEED_PASS: &str = "Enter password: \r\n";

/// Received when telnet server did not recognized last command.
static UNKNOWN_COMMAND: &str = "Unknown command\r\n\r\n";

lazy_static::lazy_static! {
    // Used to check is message can be parsed to CoturnCliResponse::Sessions.
    static ref IS_SESSIONS_REGEX: Regex =
        Regex::new(r#"Total sessions: \d"#).unwrap();

    // Used to extract session ids from CoturnCliResponse::Sessions.
    static ref EXTRACT_SESSIONS_REGEX: Regex =
        Regex::new(r"\d\) id=(.*),").unwrap();
}

/// Messages that can be received from Coturn telnet server.
#[derive(Clone, Debug)]
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
    Sessions(Vec<Session>),

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
            let (_, sessions) = parse_sessions(&msg).unwrap();

            return Ok(CoturnCliResponse::Sessions(sessions));
        }

        Err(CoturnResponseParseError::CannotDetermineResponseType(
            msg.to_owned(),
        ))
    }
}

/// Messages that can be sent to Coturn telnet client.
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
    IoError(io::Error),
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
#[derive(Default)]
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
        let mut greeting = "\r\n    1) id=007000000000000001, user
         <bb_Ralph>:\r\n      realm: medea\r\n      started 49 secs ago\r\n
         expiring in 551 secs\r\n      client protocol UDP, relay protocol
         UDP\r\n      client addr 192.168.31.183:39514, server addr
         127.0.0.1:3478\r\n      relay addr 127.0.0.1:55869\r\n
         fingerprints enforced: OFF\r\n      mobile: OFF\r\n      usage: rp=6,
         rb=480, sp=4, sb=440\r\n       rate: r=0, s=0, total=0 (bytes per
         sec)\r\n\r\n    2) id=010000000000000002, user <bb_Ralph>:\r\n
         realm: medea\r\n      started 49 secs ago\r\n      expiring in 551
         secs\r\n      client protocol TCP, relay protocol UDP\r\n      client
         addr [::1]:33710, server addr [::1]:3478\r\n      relay addr
         [::1]:60216\r\n      fingerprints enforced: OFF\r\n      mobile:
         OFF\r\n      usage: rp=4, rb=348, sp=3, sb=336\r\n       rate: r=0,
         s=0, total=0 (bytes per sec)\r\n      peers:\r\n          ::1\r\n\r\n
         3) id=000000000000000001, user <bb_Ralph>:\r\n      realm: medea\r\n
         started 49 secs ago\r\n      expiring in 551 secs\r\n      client
         protocol UDP, relay protocol UDP\r\n      client addr
         192.168.31.183:59996, server addr 127.0.0.1:3478\r\n      relay addr
         127.0.0.1:54289\r\n      fingerprints enforced: OFF\r\n      mobile:
         OFF\r\n      usage: rp=5, rb=344, sp=4, sb=440\r\n       rate: r=0,
         s=0, total=0 (bytes per sec)\r\n\r\n    4) id=005000000000000001,
         user <bb_Ralph>:\r\n      realm: medea\r\n      started 49 secs
         ago\r\n      expiring in 551 secs\r\n      client protocol TCP, relay
         protocol UDP\r\n      client addr [::1]:33712, server addr
         [::1]:3478\r\n      relay addr [::1]:52934\r\n      fingerprints
         enforced: OFF\r\n      mobile: OFF\r\n      usage: rp=12288,
         rb=10012764, sp=12288, sb=10022892\r\n       rate: r=222505,
         s=222730, total=445235 (bytes per sec)\r\n      peers:\r\n
         ::1\r\n          [::1]:62869\r\n\r\n  Total sessions: 4\r\n\r\n> "
            .into();

        match codec.decode(&mut greeting).unwrap().unwrap() {
            CoturnCliResponse::Sessions(sessions) => {
                assert_eq!(
                    sessions,
                    vec![
                        "007000000000000001",
                        "010000000000000002",
                        "000000000000000001",
                        "005000000000000001"
                    ]
                );
            }
            _ => unreachable!(),
        }
    }
}
