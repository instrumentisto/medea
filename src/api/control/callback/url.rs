//! URLs for callbacks implementation.

use std::{convert::TryFrom, fmt};

use derive_more::{Display, From};
use serde::{de::Visitor, Deserialize, Deserializer};
use url::{ParseError, Url};

/// Callback URL for gRPC client.
///
/// Note that this newtype stores only host and port of gRPC callback client
/// without anything else (protocol e.g.).
///
/// In [`Display`] implementation protocol will be added to this address.
#[derive(Clone, Debug, Display, Eq, PartialEq, Hash)]
#[display(fmt = "grpc://{}", _0)]
pub struct GrpcCallbackUrl(String);

impl GrpcCallbackUrl {
    /// Returns address for gRPC callback client.
    ///
    /// If you wish to get address with protocol - just use [`Display`]
    /// implementation.
    #[inline]
    #[must_use]
    pub fn addr(&self) -> String {
        // TODO: Do not hardcode protocol.
        format!("http://{}", self.0)
    }
}

/// All callback URLs which supported by Medea.
#[derive(Clone, derive_more::Display, Debug, Eq, PartialEq, Hash)]
pub enum CallbackUrl {
    /// gRPC callbacks type.
    Grpc(GrpcCallbackUrl),
}

/// Error of [`CallbackUrl`] parsing.
#[derive(Debug, Display, From)]
pub enum CallbackUrlParseError {
    #[display(fmt = "{:?}", _0)]
    UrlParseErr(ParseError),

    #[display(fmt = "Missing host")]
    MissingHost,

    #[display(fmt = "Unsupported URL scheme")]
    UnsupportedScheme,
}

impl TryFrom<String> for CallbackUrl {
    type Error = CallbackUrlParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let url = Url::parse(&value)?;
        let url_scheme = url.scheme();
        let host = url.host().ok_or(CallbackUrlParseError::MissingHost)?;
        let host = if let Some(port) = url.port() {
            format!("{}:{}", host, port)
        } else {
            host.to_string()
        };

        match url_scheme {
            "grpc" => Ok(Self::Grpc(GrpcCallbackUrl(host))),
            _ => Err(CallbackUrlParseError::UnsupportedScheme),
        }
    }
}

impl<'de> Deserialize<'de> for CallbackUrl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SrcUriVisitor;

        impl<'de> Visitor<'de> for SrcUriVisitor {
            type Value = CallbackUrl;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(
                    "URI to callback client in format like \
                     'grpc://127.0.0.1:9090'.",
                )
            }

            fn visit_str<E>(self, value: &str) -> Result<CallbackUrl, E>
            where
                E: serde::de::Error,
            {
                match CallbackUrl::try_from(value.to_owned()) {
                    Ok(src_uri) => Ok(src_uri),
                    Err(e) => Err(serde::de::Error::custom(e)),
                }
            }
        }

        deserializer.deserialize_identifier(SrcUriVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_parse_grpc_url() {
        for (url, expected_callback_url) in &[
            ("grpc://127.0.0.1:9090", "http://127.0.0.1:9090"),
            ("grpc://example.com:9090", "http://example.com:9090"),
            ("grpc://example.com", "http://example.com"),
            ("grpc://127.0.0.1", "http://127.0.0.1"),
        ] {
            let callback_url =
                CallbackUrl::try_from((*url).to_string()).unwrap();
            match callback_url {
                CallbackUrl::Grpc(grpc_callback_url) => {
                    assert_eq!(
                        grpc_callback_url.addr(),
                        (*expected_callback_url).to_string()
                    );
                }
            }
        }
    }

    #[test]
    fn error_on_unsupported_scheme() {
        for url in &[
            "asdf://example.com:9090",
            "asdf://127.0.0.1",
            "asdf://127.0.0.1:9090",
        ] {
            let err = CallbackUrl::try_from((*url).to_string()).unwrap_err();
            match err {
                CallbackUrlParseError::UnsupportedScheme => {}
                _ => {
                    unreachable!("Unreachable error (URL = {}): {:?}", url, err)
                }
            }
        }
    }

    #[test]
    fn error_on_url_without_scheme() {
        for url in &[
            "127.0.0.1",
            "127.0.0.1:9090",
            "example.com",
            "example.com:9090",
        ] {
            let err = CallbackUrl::try_from((*url).to_string()).unwrap_err();
            match err {
                CallbackUrlParseError::UrlParseErr(e) => match e {
                    ParseError::RelativeUrlWithoutBase => {}
                    _ => unreachable!(
                        "Unreachable ParseError [URL = {}]: {:?}.",
                        url, e
                    ),
                },
                CallbackUrlParseError::MissingHost => {}
                _ => {
                    unreachable!("Unreachable error [URL = {}]: {:?}", url, err)
                }
            }
        }
    }
}
