use std::fmt;

use failure::Fail;
use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};
use serde::{
    de::{self, Deserializer, Error, Unexpected, Visitor},
    Deserialize,
};

use super::{MemberId, RoomId, WebRtcPublishId};

// TODO: Move into endpoint module and implement From<...Pub...>
//       From<...Play...>.
macro_attr! {
    /// ID of [`Room`].
    #[derive(
        Clone,
        Debug,
        Deserialize,
        Eq,
        Hash,
        PartialEq,
        NewtypeFrom!,
        NewtypeDisplay!,
    )]
    pub struct EndpointId(pub String);
}

/// Special uri with pattern `local://{room_id}/{member_id}/{endpoint_id}`.
#[derive(Clone, Debug)]
// TODO: combine with SrcUri.
pub struct LocalUri {
    /// ID of [`Room`]
    pub room_id: Option<RoomId>,
    /// ID of `Member`
    pub member_id: Option<MemberId>,
    /// Control ID of [`Endpoint`]
    pub endpoint_id: Option<EndpointId>,
}

impl LocalUri {
    pub fn is_room_id(&self) -> bool {
        self.room_id.is_some()
            && self.member_id.is_none()
            && self.endpoint_id.is_none()
    }

    pub fn is_member_id(&self) -> bool {
        self.room_id.is_some()
            && self.member_id.is_some()
            && self.endpoint_id.is_none()
    }

    pub fn is_endpoint_id(&self) -> bool {
        self.room_id.is_some()
            && self.member_id.is_some()
            && self.endpoint_id.is_some()
    }
}

#[derive(Debug, Fail)]
pub enum ParseError {
    #[fail(display = "Too many fields")]
    TooManyFields,
    #[fail(display = "Not local uri. {}", _0)]
    NotLocalUri(String),
}

pub fn parse_local_uri(text: &str) -> Result<LocalUri, ParseError> {
    let protocol_name: String = text.chars().take(8).collect();
    if protocol_name != "local://" {
        return Err(ParseError::NotLocalUri(protocol_name));
    }

    let uri_body = text.chars().skip(8).collect::<String>();
    let mut uri_body_splitted: Vec<&str> = uri_body.rsplit('/').collect();
    let uri_body_splitted_len = uri_body_splitted.len();

    if uri_body_splitted_len == 1 {
        return Ok(LocalUri {
            room_id: Some(RoomId(uri_body_splitted.pop().unwrap().to_string())),
            member_id: None,
            endpoint_id: None,
        });
    } else if uri_body_splitted_len == 2 {
        return Ok(LocalUri {
            room_id: Some(RoomId(uri_body_splitted.pop().unwrap().to_string())),
            member_id: Some(MemberId(
                uri_body_splitted.pop().unwrap().to_string(),
            )),
            endpoint_id: None,
        });
    } else if uri_body_splitted_len == 3 {
        return Ok(LocalUri {
            room_id: Some(RoomId(uri_body_splitted.pop().unwrap().to_string())),
            member_id: Some(MemberId(
                uri_body_splitted.pop().unwrap().to_string(),
            )),
            endpoint_id: Some(EndpointId(
                uri_body_splitted.pop().unwrap().to_string(),
            )),
        });
    } else {
        return Err(ParseError::TooManyFields);
    }
}

/// Serde deserializer for [`SrcUri`].
/// Deserialize URIs with pattern `local://{room_id}/{member_id}/{endpoint_id}`.
impl<'de> Deserialize<'de> for LocalUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SrcUriVisitor;

        impl<'de> Visitor<'de> for SrcUriVisitor {
            type Value = LocalUri;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(
                    "Uri in format local://room_id/member_id/endpoint_id",
                )
            }

            fn visit_str<E>(self, value: &str) -> Result<LocalUri, E>
            where
                E: de::Error,
            {
                match parse_local_uri(value) {
                    Ok(uri) => return Ok(uri),
                    Err(e) => match e {
                        ParseError::NotLocalUri(protocol_name) => {
                            Err(Error::invalid_value(
                                Unexpected::Str(&format!(
                                    "{} in {}",
                                    protocol_name, value
                                )),
                                &self,
                            ))
                        }
                        ParseError::TooManyFields => {
                            Err(Error::custom("Too many fields."))
                        }
                    },
                }
            }
        }

        deserializer.deserialize_identifier(SrcUriVisitor)
    }
}

#[cfg(test)]
mod src_uri_deserialization_tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct SrcUriTest {
        src: LocalUri,
    }

    #[inline]
    fn id<T: From<String>>(s: &str) -> T {
        T::from(s.to_string())
    }

    #[test]
    fn deserialize() {
        let valid_json_uri =
            r#"{ "src": "local://room_id/member_id/endpoint_id" }"#;
        let local_uri: SrcUriTest =
            serde_json::from_str(valid_json_uri).unwrap();

        assert_eq!(local_uri.src.member_id, id("member_id"));
        assert_eq!(local_uri.src.room_id, id("room_id"));
        assert_eq!(local_uri.src.endpoint_id, id("endpoint_id"));
    }

    #[test]
    fn return_error_when_uri_not_local() {
        let invalid_json_uri =
            r#"{ "src": "not_local://room_id/member_id/endpoint_id" }"#;
        match serde_json::from_str::<SrcUriTest>(invalid_json_uri) {
            Ok(_) => assert!(false),
            Err(_) => assert!(true),
        }
    }

    #[test]
    fn return_error_when_uri_is_not_full() {
        let invalid_json_uri = r#"{ "src": "local://room_id/member_id" }"#;
        match serde_json::from_str::<SrcUriTest>(invalid_json_uri) {
            Ok(_) => assert!(false),
            Err(_) => assert!(true),
        }
    }

    #[test]
    fn return_error_when_uri_have_empty_part() {
        let invalid_json_uri = r#"{ "src": "local://room_id//endpoint_id" }"#;
        match serde_json::from_str::<SrcUriTest>(invalid_json_uri) {
            Ok(_) => assert!(false),
            Err(_) => assert!(true),
        }
    }
}
