use std::{convert::TryFrom, fmt};

use derive_more::Display;
use failure::_core::fmt::{Error, Formatter};
use serde::{de::Visitor, Deserialize, Deserializer};
use std::fmt::Display;
use url::{ParseError, Url};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct GrpcCallbackUrl(String);

impl GrpcCallbackUrl {
    pub fn addr(&self) -> &str {
        &self.0
    }
}

impl Display for GrpcCallbackUrl {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "grpc://{}", self.0)
    }
}

#[derive(Clone, derive_more::Display, Debug, Eq, PartialEq, Hash)]
pub enum CallbackUrl {
    Grpc(GrpcCallbackUrl),
}

#[derive(Debug, Display)]
pub enum CallbackUrlParseError {
    #[display(fmt = "{:?}", _0)]
    UrlParseErr(ParseError),

    #[display(fmt = "Missing host.")]
    MissingHost,

    #[display(fmt = "Unsupported URL scheme.")]
    UnsupportedScheme,
}

impl From<ParseError> for CallbackUrlParseError {
    fn from(err: ParseError) -> Self {
        Self::UrlParseErr(err)
    }
}

impl TryFrom<String> for CallbackUrl {
    type Error = CallbackUrlParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let url = Url::parse(&value)?;
        let url_scheme = url.scheme();
        let host = url
            .host()
            .ok_or_else(|| CallbackUrlParseError::MissingHost)?;
        let host = if let Some(port) = url.port() {
            format!("{}:{}", host, port)
        } else {
            host.to_string()
        };

        match url_scheme {
            "grpc" => Ok(CallbackUrl::Grpc(GrpcCallbackUrl(host))),
            _ => Err(CallbackUrlParseError::UnsupportedScheme),
        }
    }
}

/// [Serde] deserializer for [`CallbackUrl`].
///
/// [Serde]: serde
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
                    "URI to callback service in format like \
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
            ("grpc://127.0.0.1:9090", "127.0.0.1:9090"),
            ("grpc://example.com:9090", "example.com:9090"),
            ("grpc://example.com", "example.com"),
            ("grpc://127.0.0.1", "127.0.0.1"),
        ] {
            let callback_url = CallbackUrl::try_from(url.to_string()).unwrap();
            match callback_url {
                CallbackUrl::Grpc(grpc_callback_url) => {
                    assert_eq!(
                        grpc_callback_url.to_string(),
                        expected_callback_url.to_string()
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
            let res = if let Err(e) = CallbackUrl::try_from(url.to_string()) {
                e
            } else {
                unreachable!(
                    "Unreachable successful result of parsing. {}",
                    url
                );
            };
            match res {
                CallbackUrlParseError::UnsupportedScheme => {}
                _ => {
                    unreachable!("Unreachable error (URL = {}): {:?}", url, res)
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
            let res = if let Err(e) = CallbackUrl::try_from(url.to_string()) {
                e
            } else {
                unreachable!(
                    "Unreachable successful result of parsing. {}",
                    url
                );
            };
            match res {
                CallbackUrlParseError::UrlParseErr(e) => match e {
                    ParseError::RelativeUrlWithoutBase => {}
                    _ => unreachable!(
                        "Unreachable ParseError [URL = {}]: {:?}.",
                        url, e
                    ),
                },
                CallbackUrlParseError::MissingHost => {}
                _ => {
                    unreachable!("Unreachable error [URL = {}]: {:?}", url, res)
                }
            }
        }
    }
}
