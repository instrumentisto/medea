//! Control API specification Endpoint definitions.

use std::{convert::TryFrom, fmt};

use serde::{
    de::{self, Deserializer, Error, Visitor},
    Deserialize,
};

use crate::api::control::MemberId;

use super::{Entity, TryFromEntityError};
use serde::de::Unexpected;

/// [`Endpoint`] represents a media element that one or more media data streams
/// flow through.
#[derive(Debug)]
pub enum Endpoint {
    WebRtcPublish(WebRtcPublishEndpoint),
    WebRtcPlay(WebRtcPlayEndpoint),
}

impl TryFrom<Entity> for Endpoint {
    type Error = TryFromEntityError;

    fn try_from(from: Entity) -> Result<Self, Self::Error> {
        match from {
            Entity::WebRtcPlayEndpoint { spec } => {
                Ok(Endpoint::WebRtcPlay(spec))
            }
            Entity::WebRtcPublishEndpoint { spec } => {
                Ok(Endpoint::WebRtcPublish(spec))
            }
            _ => Err(TryFromEntityError::NotEndpoint),
        }
    }
}

/// Peer-to-peer mode of [`WebRtcPublishEndpoint`].
#[derive(Clone, Deserialize, Debug)]
pub enum P2pMode {
    /// Always connect peer-to-peer.
    Always,
}

/// Media element which is able to publish media data for another client via
/// WebRTC.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Deserialize, Debug)]
pub struct WebRtcPublishEndpoint {
    /// Peer-to-peer mode.
    pub p2p: P2pMode,
}

/// Media element which is able to play media data for client via WebRTC.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Deserialize, Debug)]
pub struct WebRtcPlayEndpoint {
    /// Source URI in format `local://{room_id}/{member_id}/{pipeline_id}`.
    pub src: LocalUri,
}

/// Special uri with pattern `local://{room_id}/{member_id}/{pipeline_id}`.
#[derive(Clone, Debug)]
pub struct LocalUri {
    /// ID of [`Room`]
    // TODO: Why this field never used???
    pub room_id: String,
    /// ID of [`Member`]
    pub member_id: MemberId,
    /// Control ID of [`Endpoint`]
    pub pipeline_id: String,
}

/// Serde deserializer for [`LocalUri`].
/// Deserialize URIs with pattern `local://{room_id}/{member_id}/{pipeline_id}`.
impl<'de> Deserialize<'de> for LocalUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LocalUriVisitor;

        impl<'de> Visitor<'de> for LocalUriVisitor {
            type Value = LocalUri;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(
                    "Uri in format local://room_id/member_id/pipeline_id",
                )
            }

            fn visit_str<E>(self, value: &str) -> Result<LocalUri, E>
            where
                E: de::Error,
            {
                let protocol_name: String = value.chars().take(8).collect();
                if protocol_name != "local://" {
                    return Err(Error::invalid_value(
                        Unexpected::Str(&format!(
                            "{} in {}",
                            protocol_name, value
                        )),
                        &self,
                    ));
                }

                let uri_body = value.chars().skip(8).collect::<String>();
                let mut uri_body_splitted: Vec<&str> =
                    uri_body.rsplit('/').collect();
                let uri_body_splitted_len = uri_body_splitted.len();
                if uri_body_splitted_len != 3 {
                    let error_msg = if uri_body_splitted_len == 0 {
                        "room_id, member_id, pipeline_id"
                    } else if uri_body_splitted_len == 1 {
                        "member_id, pipeline_id"
                    } else if uri_body_splitted_len == 2 {
                        "pipeline_id"
                    } else {
                        return Err(Error::custom(format!(
                            "Too many fields: {}. Expecting 3 fields, found \
                             {}.",
                            uri_body, uri_body_splitted_len
                        )));
                    };
                    return Err(Error::missing_field(error_msg));
                }
                let room_id = uri_body_splitted.pop().unwrap().to_string();
                if room_id.is_empty() {
                    return Err(Error::custom(format!(
                        "room_id in {} is empty!",
                        value
                    )));
                }
                let member_id = uri_body_splitted.pop().unwrap().to_string();
                if member_id.is_empty() {
                    return Err(Error::custom(format!(
                        "member_id in {} is empty!",
                        value
                    )));
                }
                let pipeline_id = uri_body_splitted.pop().unwrap().to_string();
                if pipeline_id.is_empty() {
                    return Err(Error::custom(format!(
                        "pipeline_id in {} is empty!",
                        value
                    )));
                }

                Ok(LocalUri {
                    room_id,
                    member_id: MemberId(member_id),
                    pipeline_id,
                })
            }
        }

        deserializer.deserialize_identifier(LocalUriVisitor)
    }
}

#[cfg(test)]
mod test {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct LocalUriTest {
        src: LocalUri,
    }

    #[test]
    fn should_parse_local_uri() {
        let valid_json_uri =
            r#"{ "src": "local://room_id/member_id/pipeline_id" }"#;
        let local_uri: LocalUriTest =
            serde_json::from_str(valid_json_uri).unwrap();

        assert_eq!(
            local_uri.src.member_id,
            MemberId(String::from("member_id"))
        );
        assert_eq!(local_uri.src.room_id, String::from("room_id"));
        assert_eq!(local_uri.src.pipeline_id, String::from("pipeline_id"));
    }

    #[test]
    fn should_return_error_when_uri_not_local() {
        let invalid_json_uri =
            r#"{ "src": "not_local://room_id/member_id/pipeline_id" }"#;
        match serde_json::from_str::<LocalUriTest>(invalid_json_uri) {
            Ok(_) => assert!(false),
            Err(_) => assert!(true),
        }
    }

    #[test]
    fn should_return_error_when_uri_is_not_full() {
        let invalid_json_uri = r#"{ "src": "local://room_id/member_id" }"#;
        match serde_json::from_str::<LocalUriTest>(invalid_json_uri) {
            Ok(_) => assert!(false),
            Err(_) => assert!(true),
        }
    }

    #[test]
    fn should_return_error_when_uri_have_empty_part() {
        let invalid_json_uri = r#"{ "src": "local://room_id//pipeline_id" }"#;
        match serde_json::from_str::<LocalUriTest>(invalid_json_uri) {
            Ok(_) => assert!(false),
            Err(_) => assert!(true),
        }
    }
}
