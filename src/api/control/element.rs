//! Control API specification Element definitions.

use serde::{
    de::{self, Deserializer, Error, Visitor},
    Deserialize,
};

use std::{convert::TryFrom, fmt};

use super::{Entity, TryFromEntityError};

/// [`Element`] represents a media element that one or more media data streams
/// flow through.
#[derive(Debug)]
pub enum Element {
    WebRtcPublishEndpoint(WebRtcPublishEndpoint),
    WebRtcPlayEndpoint(WebRtcPlayEndpoint),
}
impl TryFrom<Entity> for Element {
    type Error = TryFromEntityError;

    fn try_from(from: Entity) -> Result<Self, Self::Error> {
        match from {
            Entity::WebRtcPlayEndpoint { spec } => {
                Ok(Element::WebRtcPlayEndpoint(spec))
            }
            Entity::WebRtcPublishEndpoint { spec } => {
                Ok(Element::WebRtcPublishEndpoint(spec))
            }
            _ => Err(TryFromEntityError::NotElement),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
/// Peer-to-peer mode of [`WebRtcPublishEndpoint`].
pub enum P2pMode {
    /// Always connect peer-to-peer.
    Always,
}

#[derive(Deserialize, Debug, Clone)]
/// Media element which is able to publish media data for another client via
/// WebRTC.
pub struct WebRtcPublishEndpoint {
    /// Peer-to-peer mode.
    pub p2p: P2pMode,
}

/// Media element which is able to play media data for client via WebRTC.
#[derive(Deserialize, Debug, Clone)]
pub struct WebRtcPlayEndpoint {
    /// Source URI in format `local://{room_id}/{member_id}/{pipeline_id}`.
    pub src: LocalUri,
}

#[derive(Debug, Clone)]
/// Special uri with pattern `local://{room_id}/{member_id}/{pipeline_id}`.
pub struct LocalUri {
    /// ID of [`Room`]
    // TODO: Why this field never used???
    pub room_id: String,
    /// ID of [`Member`]
    pub member_id: String,
    /// Control ID of [`Element`]
    pub pipeline_id: String,
}

/// Serde deserializer for [`LocalUri`].
/// Deserialize URIs with pattern `local://{room_id}/{member_id}/{pipeline_id}`.
impl<'de> Deserialize<'de> for LocalUri {
    fn deserialize<D>(deserializer: D) -> Result<LocalUri, D::Error>
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
                    return Err(Error::custom(
                        "Expected local uri in format \
                         local://room_id/member_id/pipeline_id!",
                    ));
                }

                let uri_body = value.chars().skip(8).collect::<String>();
                let mut uri_body_splitted: Vec<&str> =
                    uri_body.rsplit('/').collect();
                let uri_body_splitted_len = uri_body_splitted.len();
                if uri_body_splitted_len != 3 {
                    let error_msg = if uri_body_splitted_len == 0 {
                        "Missing room_id, member_id, pipeline_id"
                    } else if uri_body_splitted_len == 1 {
                        "Missing member_id, pipeline_id"
                    } else if uri_body_splitted_len == 2 {
                        "Missing pipeline_id"
                    } else {
                        "Too many params"
                    };
                    return Err(Error::custom(error_msg));
                }
                let room_id = uri_body_splitted.pop().unwrap().to_string();
                if room_id.is_empty() {
                    return Err(Error::custom("room_id is empty!"));
                }
                let member_id = uri_body_splitted.pop().unwrap().to_string();
                if member_id.is_empty() {
                    return Err(Error::custom("member_id is empty!"));
                }
                let pipeline_id = uri_body_splitted.pop().unwrap().to_string();
                if pipeline_id.is_empty() {
                    return Err(Error::custom("pipeline_id is empty!"));
                }

                Ok(LocalUri {
                    room_id,
                    member_id,
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

        assert_eq!(local_uri.src.member_id, "member_id".to_string());
        assert_eq!(local_uri.src.room_id, "room_id".to_string());
        assert_eq!(local_uri.src.pipeline_id, "pipeline_id".to_string());
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
