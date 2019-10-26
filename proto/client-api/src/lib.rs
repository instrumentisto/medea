//! Client API protocol implementation for Medea media server.

use std::collections::HashMap;

use derive_more::{Constructor, Display};
use medea_macro::dispatchable;
use serde::{de::Deserializer, ser::Serializer, Deserialize, Serialize};

/// ID of `Peer`.
#[cfg_attr(
    feature = "medea",
    derive(Deserialize, Debug, Hash, Eq, Default, PartialEq)
)]
#[cfg_attr(feature = "jason", derive(Serialize))]
#[derive(Clone, Copy, Display)]
pub struct PeerId(pub u64);

/// ID of `MediaTrack`.
#[cfg_attr(
    feature = "medea",
    derive(Deserialize, Debug, Hash, Eq, Default, PartialEq)
)]
#[cfg_attr(feature = "jason", derive(Serialize))]
#[derive(Clone, Copy, Display)]
pub struct TrackId(pub u64);

/// Value that is able to be incremented by `1`.
#[cfg(feature = "medea")]
pub trait Incrementable {
    /// Returns current value + 1.
    fn incr(&self) -> Self;
}

/// Implements [`Incrementable`] trait for newtype with any numeric type.
macro_rules! impl_incrementable {
    ($name:ty) => {
        impl Incrementable for $name {
            fn incr(&self) -> Self {
                Self(self.0 + 1)
            }
        }
    };
}

#[cfg(feature = "medea")]
impl_incrementable!(PeerId);
#[cfg(feature = "medea")]
impl_incrementable!(TrackId);

// TODO: should be properly shared between medea and jason
#[allow(dead_code)]
#[cfg_attr(test, derive(Debug, PartialEq))]
/// Message sent by `Media Server` to `Client`.
pub enum ServerMsg {
    /// `pong` message that server answers with to WebSocket client in response
    /// to received `ping` message.
    Pong(u64),
    /// `Media Server` notifies `Client` about happened facts and it reacts on
    /// them to reach the proper state.
    Event(Event),
}

#[allow(dead_code)]
#[cfg_attr(test, derive(Debug, PartialEq))]
/// Message from 'Client' to 'Media Server'.
pub enum ClientMsg {
    /// `ping` message that WebSocket client is expected to send to the server
    /// periodically.
    Ping(u64),
    /// Request of `Web Client` to change the state on `Media Server`.
    Command(Command),
}

/// WebSocket message from Web Client to Media Server.
#[allow(dead_code)]
#[cfg_attr(feature = "medea", derive(Deserialize))]
#[cfg_attr(feature = "jason", derive(Serialize))]
#[cfg_attr(test, derive(Debug, PartialEq))]
#[serde(tag = "command", content = "data")]
pub enum Command {
    /// Web Client sends SDP Offer.
    MakeSdpOffer {
        peer_id: PeerId,
        sdp_offer: String,
        /// Associations between [`Track`] and transceiver's [media
        /// description][1].
        ///
        /// `mid` is basically an ID of [`m=<media>` section][1] in SDP.
        ///
        /// [1]: https://tools.ietf.org/html/rfc4566#section-5.14
        mids: HashMap<TrackId, String>,
    },
    /// Web Client sends SDP Answer.
    MakeSdpAnswer { peer_id: PeerId, sdp_answer: String },
    /// Web Client sends Ice Candidate.
    SetIceCandidate {
        peer_id: PeerId,
        candidate: IceCandidate,
    },
}

/// Reason of disconnecting Web Client from Media Server.
#[derive(Debug, Deserialize, Serialize)]
pub enum CloseReason {
    /// Client session was finished on a server side.
    Finished,

    /// Old connection was closed due to a client reconnection.
    Reconnected,

    /// Connection has been inactive for a while and thus considered idle
    /// by a server.
    Idle,

    /// Establishing of connection with a server was rejected on server side.
    ///
    /// Most likely because of incorrect Member credentials.
    Rejected,

    /// Server internal error has occurred while connecting.
    ///
    /// This close reason is similar to 500 HTTP status code.
    InternalError,

    /// Client was evicted on the server side.
    Evicted,
}

/// Description which is sent in [Close] WebSocket frame from Media Server
/// to Web Client.
///
/// [Close]: https://tools.ietf.org/html/rfc6455#section-5.5.1
#[derive(Constructor, Debug, Deserialize, Serialize)]
pub struct CloseDescription {
    /// Reason of why WebSocket connection has been closed.
    pub reason: CloseReason,
}

/// WebSocket message from Medea to Jason.
#[allow(dead_code)]
#[dispatchable]
#[cfg_attr(feature = "medea", derive(Serialize, Debug, Clone, PartialEq))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
#[serde(tag = "event", content = "data")]
pub enum Event {
    /// Media Server notifies Web Client about necessity of RTCPeerConnection
    /// creation.
    PeerCreated {
        peer_id: PeerId,
        sdp_offer: Option<String>,
        tracks: Vec<Track>,
        ice_servers: Vec<IceServer>,
    },
    /// Media Server notifies Web Client about necessity to apply specified SDP
    /// Answer to Web Client's RTCPeerConnection.
    SdpAnswerMade { peer_id: PeerId, sdp_answer: String },

    /// Media Server notifies Web Client about necessity to apply specified
    /// ICE Candidate.
    IceCandidateDiscovered {
        peer_id: PeerId,
        candidate: IceCandidate,
    },

    /// Media Server notifies Web Client about necessity of RTCPeerConnection
    /// close.
    PeersRemoved { peer_ids: Vec<PeerId> },
}

/// Represents [RTCIceCandidateInit][1] object.
///
/// [1]: https://www.w3.org/TR/webrtc/#dom-rtcicecandidateinit
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct IceCandidate {
    pub candidate: String,
    pub sdp_m_line_index: Option<u16>,
    pub sdp_mid: Option<String>,
}

/// [`Track`] with specified direction.
#[cfg_attr(feature = "medea", derive(Serialize, Debug, Clone, PartialEq))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
pub struct Track {
    pub id: TrackId,
    pub direction: Direction,
    pub media_type: MediaType,
}

/// Representation of [RTCIceServer][1] (item of `iceServers` field
/// from [RTCConfiguration][2]).
///
/// [1]: https://developer.mozilla.org/en-US/docs/Web/API/RTCIceServer
/// [2]: https://developer.mozilla.org/en-US/docs/Web/API/RTCConfiguration
#[derive(Clone, Debug)]
#[cfg_attr(feature = "medea", derive(Serialize, PartialEq))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

/// Direction of [`Track`].
#[cfg_attr(feature = "medea", derive(Serialize, Debug, Clone, PartialEq))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
// TODO: Use different struct without mids in TracksApplied event.
pub enum Direction {
    Send {
        receivers: Vec<PeerId>,
        mid: Option<String>,
    },
    Recv {
        sender: PeerId,
        mid: Option<String>,
    },
}

/// Type of [`Track`].
#[cfg_attr(feature = "medea", derive(Serialize, Debug, PartialEq, Clone))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
pub enum MediaType {
    Audio(AudioSettings),
    Video(VideoSettings),
}

#[cfg_attr(feature = "medea", derive(Serialize, Clone, Debug, PartialEq))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
pub struct AudioSettings {}

#[cfg_attr(feature = "medea", derive(Serialize, Clone, Debug, PartialEq))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
pub struct VideoSettings {}

#[cfg(feature = "jason")]
impl Serialize for ClientMsg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        match self {
            Self::Ping(n) => {
                let mut ping = serializer.serialize_struct("ping", 1)?;
                ping.serialize_field("ping", n)?;
                ping.end()
            }
            Self::Command(command) => command.serialize(serializer),
        }
    }
}

#[cfg(feature = "medea")]
impl<'de> Deserialize<'de> for ClientMsg {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        let ev = serde_json::Value::deserialize(deserializer)?;
        let map = ev.as_object().ok_or_else(|| {
            Error::custom(format!("unable to deser ClientMsg [{:?}]", &ev))
        })?;

        if let Some(v) = map.get("ping") {
            let n = v.as_u64().ok_or_else(|| {
                Error::custom(format!(
                    "unable to deser ClientMsg::Ping [{:?}]",
                    &ev
                ))
            })?;

            Ok(Self::Ping(n))
        } else {
            let command =
                serde_json::from_value::<Command>(ev).map_err(|e| {
                    Error::custom(format!(
                        "unable to deser ClientMsg::Command [{:?}]",
                        e
                    ))
                })?;
            Ok(Self::Command(command))
        }
    }
}

#[cfg(feature = "medea")]
impl Serialize for ServerMsg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        match self {
            Self::Pong(n) => {
                let mut ping = serializer.serialize_struct("pong", 1)?;
                ping.serialize_field("pong", n)?;
                ping.end()
            }
            Self::Event(command) => command.serialize(serializer),
        }
    }
}

#[cfg(feature = "jason")]
impl<'de> Deserialize<'de> for ServerMsg {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        let ev = serde_json::Value::deserialize(deserializer)?;
        let map = ev.as_object().ok_or_else(|| {
            Error::custom(format!("unable to deser ServerMsg [{:?}]", &ev))
        })?;

        if let Some(v) = map.get("pong") {
            let n = v.as_u64().ok_or_else(|| {
                Error::custom(format!(
                    "unable to deser ServerMsg::Pong [{:?}]",
                    &ev
                ))
            })?;

            Ok(Self::Pong(n))
        } else {
            let event = serde_json::from_value::<Event>(ev).map_err(|e| {
                Error::custom(format!(
                    "unable to deser ServerMsg::Event [{:?}]",
                    e
                ))
            })?;
            Ok(Self::Event(event))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn command() {
        let mut mids = HashMap::new();
        mids.insert(TrackId(0), String::from("1"));

        let command = ClientMsg::Command(Command::MakeSdpOffer {
            peer_id: PeerId(77),
            sdp_offer: "offer".to_owned(),
            mids,
        });
        #[cfg_attr(nightly, rustfmt::skip)]
            let command_str =
            "{\
                \"command\":\"MakeSdpOffer\",\
                \"data\":{\
                    \"peer_id\":77,\
                    \"sdp_offer\":\"offer\",\
                    \"mids\":{\"0\":\"1\"}\
                }\
            }";

        assert_eq!(command_str, serde_json::to_string(&command).unwrap());
        assert_eq!(
            command,
            serde_json::from_str(&serde_json::to_string(&command).unwrap())
                .unwrap()
        );
    }

    #[test]
    fn ping() {
        let ping = ClientMsg::Ping(15);
        let ping_str = "{\"ping\":15}";

        assert_eq!(ping_str, serde_json::to_string(&ping).unwrap());
        assert_eq!(
            ping,
            serde_json::from_str(&serde_json::to_string(&ping).unwrap())
                .unwrap()
        )
    }

    #[test]
    fn event() {
        let event = ServerMsg::Event(Event::SdpAnswerMade {
            peer_id: PeerId(45),
            sdp_answer: "answer".to_owned(),
        });
        #[cfg_attr(nightly, rustfmt::skip)]
            let event_str =
            "{\
                \"event\":\"SdpAnswerMade\",\
                \"data\":{\
                    \"peer_id\":45,\
                    \"sdp_answer\":\"answer\"\
                }\
            }";

        assert_eq!(event_str, serde_json::to_string(&event).unwrap());
        assert_eq!(
            event,
            serde_json::from_str(&serde_json::to_string(&event).unwrap())
                .unwrap()
        );
    }

    #[test]
    fn pong() {
        let pong = ServerMsg::Pong(5);
        let pong_str = "{\"pong\":5}";

        assert_eq!(pong_str, serde_json::to_string(&pong).unwrap());
        assert_eq!(
            pong,
            serde_json::from_str(&serde_json::to_string(&pong).unwrap())
                .unwrap()
        )
    }
}
