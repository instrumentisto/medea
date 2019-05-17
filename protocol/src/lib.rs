use serde::{de::Deserializer, ser::Serializer, Deserialize, Serialize};
use medea_derives::EventDispatcher;

// TODO: should be properly shared between medea and jason
#[cfg_attr(test, derive(PartialEq, Debug))]
#[allow(dead_code)]
/// Message sent by `Media Server` to `Client`.
pub enum ServerMsg {
    /// `pong` message that server answers with to WebSocket client in response
    /// to received `ping` message.
    Pong(u64),
    /// `Media Server` notifies `Client` about happened facts and it reacts on
    /// them to reach the proper state.
    Event(Event),
}

#[cfg_attr(test, derive(PartialEq, Debug))]
#[allow(dead_code)]
/// Message from 'Client' to 'Media Server'.
pub enum ClientMsg {
    /// `ping` message that WebSocket client is expected to send to the server
    /// periodically.
    Ping(u64),
    /// Request of `Web Client` to change the state on `Media Server`.
    Command(Command),
}

/// WebSocket message from Web Client to Media Server.
#[derive(Deserialize, Serialize)]
#[serde(tag = "command", content = "data")]
#[cfg_attr(test, derive(PartialEq, Debug))]
#[allow(dead_code)]
pub enum Command {
    /// Web Client sends SDP Offer.
    MakeSdpOffer { peer_id: u64, sdp_offer: String },
    /// Web Client sends SDP Answer.
    MakeSdpAnswer { peer_id: u64, sdp_answer: String },
    /// Web Client sends Ice Candidate.
    SetIceCandidate {
        peer_id: u64,
        candidate: IceCandidate,
    },
}

/// WebSocket message from Medea to Jason.
#[derive(Deserialize, Serialize, Clone, EventDispatcher)]
#[serde(tag = "event", content = "data")]
#[cfg_attr(test, derive(PartialEq, Debug))]
#[allow(dead_code)]
pub enum Event {
    /// Media Server notifies Web Client about necessity of RTCPeerConnection
    /// creation.
    PeerCreated {
        peer_id: u64,
        sdp_offer: Option<String>,
        tracks: Vec<Track>,
    },
    /// Media Server notifies Web Client about necessity to apply specified SDP
    /// Answer to Web Client's RTCPeerConnection.
    SdpAnswerMade {
        peer_id: u64, sdp_answer: String
    },

    /// Media Server notifies Web Client about necessity to apply specified
    /// ICE Candidate.
    IceCandidateDiscovered {
        peer_id: u64,
        candidate: IceCandidate,
    },

    /// Media Server notifies Web Client about necessity of RTCPeerConnection
    /// close.
    PeersRemoved {
        peer_ids: Vec<u64>
    },
}

/// Represents [`RtcIceCandidateInit`] object.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct IceCandidate {
    pub candidate: String,
    pub sdp_m_line_index: Option<u16>,
    pub sdp_mid: Option<String>,
}

/// [`Track] with specified direction.
#[derive(Deserialize, Serialize, Clone)]
#[cfg_attr(test, derive(PartialEq, Debug))]
pub struct Track {
    pub id: u64,
    pub direction: Direction,
    pub media_type: MediaType,
}

/// Direction of [`Track`].
#[derive(Deserialize, Serialize, Clone)]
#[cfg_attr(test, derive(PartialEq, Debug))]
pub enum Direction {
    Send { receivers: Vec<u64> },
    Recv { sender: u64 },
}

/// Type of [`Track`].
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(test, derive(PartialEq))]
pub enum MediaType {
    Audio(AudioSettings),
    Video(VideoSettings),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct AudioSettings {}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct VideoSettings {}

impl Serialize for ClientMsg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        match self {
            ClientMsg::Ping(n) => {
                let mut ping = serializer.serialize_struct("ping", 1)?;
                ping.serialize_field("ping", n)?;
                ping.end()
            }
            ClientMsg::Command(command) => command.serialize(serializer),
        }
    }
}

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

            Ok(ClientMsg::Ping(n))
        } else {
            let command =
                serde_json::from_value::<Command>(ev).map_err(|e| {
                    Error::custom(format!(
                        "unable to deser ClientMsg::Command [{:?}]",
                        e
                    ))
                })?;
            Ok(ClientMsg::Command(command))
        }
    }
}

impl Serialize for ServerMsg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        match self {
            ServerMsg::Pong(n) => {
                let mut ping = serializer.serialize_struct("pong", 1)?;
                ping.serialize_field("pong", n)?;
                ping.end()
            }
            ServerMsg::Event(command) => command.serialize(serializer),
        }
    }
}

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

            Ok(ServerMsg::Pong(n))
        } else {
            let event = serde_json::from_value::<Event>(ev).map_err(|e| {
                Error::custom(format!(
                    "unable to deser ServerMsg::Event [{:?}]",
                    e
                ))
            })?;
            Ok(ServerMsg::Event(event))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn command() {
        let command = ClientMsg::Command(Command::MakeSdpOffer {
            peer_id: 77,
            sdp_offer: "offer".to_owned(),
        });
        #[cfg_attr(nightly, rustfmt::skip)]
            let command_str =
            "{\
                \"command\":\"MakeSdpOffer\",\
                \"data\":{\
                    \"peer_id\":77,\
                    \"sdp_offer\":\"offer\"\
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
            peer_id: 45,
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
