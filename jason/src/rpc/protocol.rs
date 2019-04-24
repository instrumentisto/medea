use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

// TODO: should be properly shared between medea and jason
#[derive(Deserialize)]
#[allow(dead_code)]
pub enum ServerMsg {
    /// `pong` message that server answers with to WebSocket client in response
    /// to received `ping` message.
    Pong(usize),
    Event(Event),
}

#[allow(dead_code)]
pub enum ClientMsg {
    /// `ping` message that WebSocket client is expected to send to the server
    /// periodically.
    Ping(usize),
    Command(Command),
}

impl Serialize for ClientMsg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::{SerializeStruct};

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

impl Serialize for ServerMsg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::{SerializeStruct};

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

/// WebSocket message from Web Client to Media Server.
#[derive(Deserialize, Serialize)]
#[serde(tag = "command", content = "data")]
#[allow(dead_code)]
pub enum Command {
    /// Web Client sends SDP Offer.
    MakeSdpOffer { peer_id: u64, sdp_offer: String },
    /// Web Client sends SDP Answer.
    MakeSdpAnswer { peer_id: u64, sdp_answer: String },
    /// Web Client sends Ice Candidate.
    SetIceCandidate { peer_id: u64, candidate: String },
}

/// WebSocket message from Medea to Jason.
#[derive(Deserialize, Serialize)]
#[serde(tag = "event", content = "data")]
#[allow(dead_code)]
pub enum Event {
    /// Media Server notifies Web Client about necessity of RTCPeerConnection
    /// creation.
    PeerCreated {
        peer_id: u64,
        sdp_offer: Option<String>,
        tracks: Vec<DirectionalTrack>,
    },
    /// Media Server notifies Web Client about necessity to apply specified SDP
    /// Answer to Web Client's RTCPeerConnection.
    SdpAnswerMade {
        peer_id: u64,
        sdp_answer: String,
    },

    IceCandidateDiscovered {
        peer_id: u64,
        candidate: String,
    },

    /// Media Server notifies Web Client about necessity of RTCPeerConnection
    /// close.
    PeersRemoved {
        peer_ids: Vec<u64>,
    },
}

/// [`Track] with specified direction.
#[derive(Deserialize, Serialize)]
pub struct DirectionalTrack {
    pub id: u64,
    pub direction: TrackDirection,
    pub media_type: TrackMediaType,
}

/// Direction of [`Track`].
#[derive(Deserialize, Serialize)]
pub enum TrackDirection {
    Send { receivers: Vec<u64> },
    Recv { sender: u64 },
}

/// Type of [`Track`].
#[derive(Deserialize, Serialize)]
pub enum TrackMediaType {
    Audio(AudioSettings),
    Video(VideoSettings),
}

#[derive(Deserialize, Serialize)]
pub struct AudioSettings {}

#[derive(Deserialize, Serialize)]
pub struct VideoSettings {}

#[cfg(test)]
mod test {
    use crate::rpc::protocol::Command::MakeSdpOffer;
    use crate::rpc::protocol::Event::SdpAnswerMade;
    use crate::rpc::protocol::{ClientMsg, ServerMsg};

    #[test]
    fn serialize_ping() {
        assert_eq!(
            r#"{"ping":5}"#,
            serde_json::to_string(&ClientMsg::Ping(5)).unwrap()
        );
    }

    #[test]
    fn serialize_pong() {
        assert_eq!(
            r#"{"pong":10}"#,
            serde_json::to_string(&ServerMsg::Pong(10)).unwrap()
        );
    }

    #[test]
    fn serialize_command() {
        let command =
            serde_json::to_string(&ClientMsg::Command(MakeSdpOffer {
                peer_id: 5,
                sdp_offer: "offer".to_owned(),
            }))
            .unwrap();
        #[cfg_attr(nightly, rustfmt::skip)]
        assert_eq!(
            command,
            "{\
               \"command\":\"MakeSdpOffer\",\
               \"data\":{\
                 \"peer_id\":5,\
                 \"sdp_offer\":\"offer\"\
               }\
             }",
        );
    }

    #[test]
    fn serialize_event() {
        let event = serde_json::to_string(&ServerMsg::Event(SdpAnswerMade {
            peer_id: 45,
            sdp_answer: "answer".to_owned(),
        }))
        .unwrap();
        #[cfg_attr(nightly, rustfmt::skip)]
        assert_eq!(
            event,
            "{\
               \"event\":\"SdpAnswerMade\",\
               \"data\":{\
                 \"peer_id\":45,\
                 \"sdp_answer\":\"answer\"\
               }\
             }",
        );
    }
}
