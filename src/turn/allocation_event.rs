//! Implementation of the Coturn events deserialization.

use std::{
    borrow::Cow, collections::HashMap, num::ParseIntError, time::Duration,
};

use derive_more::Display;
use medea_client_api_proto::{PeerId, RoomId};
use redis::RedisError;

/// Errors of [`CoturnEvent`] parsing.
#[derive(Debug, Display)]
pub enum CoturnEventParseError {
    /// Unsupported allocation status.
    #[display(fmt = "Unsupported allocation status: {}", _0)]
    UnsupportedStatus(String),

    /// Unsupported allocation event type.
    #[display(fmt = "Unsupported allocation event type: {}", _0)]
    UnsupportedEventType(String),

    /// Some traffic stats event's field not found.
    #[display(fmt = "Field {} not found in traffic event", _0)]
    FieldNotFoundInTrafficUpdate(Cow<'static, str>),

    /// Failed to parse traffic event stat metadata.
    #[display(
        fmt = "Failed to parse traffic stat '{}' from traffic event: {:?}",
        _0,
        _1
    )]
    FailedToParseTrafficMap(String, ParseIntError),

    /// Traffic stat map from traffic event has incorrect formatting.
    #[display(
        fmt = "Traffic stat '{}' map from traffic event has incorrect \
               formatting",
        _0
    )]
    IncorrectTrafficMapFormat(String),

    /// Status is empty.
    #[display(fmt = "Status is empty")]
    EmptyStatus,

    /// Status doesn't have metadata.
    #[display(fmt = "Status doesn't have metadata")]
    NoMetadataInStatus,

    /// Allocation lifetime parsing failed.
    #[display(fmt = "Allocation lifetime parsing failed: {:?}", _0)]
    FailedLifetimeParsing(ParseIntError),

    /// Redis channel info is empty.
    #[display(fmt = "Redis channel info is empty: {:?}", _0)]
    NoChannelInfo(RedisError),

    /// No user metadata.
    #[display(fmt = "No user metadata")]
    NoUserInfo,

    /// No [`MemberId`] metadata.
    ///
    /// [`MemberId`]: medea_client_api_proto::MemberId
    #[display(fmt = "No MemberId metadata")]
    NoMemberId,

    /// No [`PeerId`] metadata.
    #[display(fmt = "No PeerId metadata")]
    NoPeerId,

    /// Failed to parse [`PeerId`] metadata.
    #[display(fmt = "Failed to parse PeerId metadata: {:?}", _0)]
    FailedPeerIdParsing(ParseIntError),

    /// No allocation ID metadata.
    #[display(fmt = "No allocation ID metadata")]
    NoAllocationId,

    /// Failed to parse allocation ID metadata.
    #[display(fmt = "Failed to parse allocation ID: {:?}", _0)]
    FailedAllocationIdParsing(ParseIntError),

    /// Event type is not provided.
    #[display(fmt = "No event type")]
    NoEventType,

    /// Wrong Redis payload type.
    #[display(fmt = "Wrong Redis payload type: {:?}", _0)]
    WrongPayloadType(RedisError),
}

/// Allocation event received from Coturn.
#[derive(Debug)]
pub struct CoturnEvent {
    /// Actual allocation event received from Coturn.
    pub event: CoturnAllocationEvent,

    /// [`RoomId`] for which this [`CoturnEvent`] was received.
    pub room_id: RoomId,

    /// [`PeerId`] for which this [`CoturnEvent`] was received.
    pub peer_id: PeerId,

    /// Allocation ID for which this [`CoturnEvent`] was received.
    pub allocation_id: u64,
}

impl CoturnEvent {
    /// Tries to parse [`CoturnEvent]` from a provided Redis message.
    ///
    /// # Errors
    ///
    /// All [`CoturnEventParseError`] variants can be returned from this
    /// function, so read their docs.
    ///
    /// All errors from this function should never happen, so there is no sense
    /// to catch them individually.
    pub fn parse(msg: &redis::Msg) -> Result<Self, CoturnEventParseError> {
        use CoturnEventParseError as E;

        let channel: String = msg.get_channel().map_err(E::NoChannelInfo)?;
        let mut channel_splitted = channel.split('/').skip(4);

        let (room_id, peer_id) = {
            let user = channel_splitted.next().ok_or(E::NoUserInfo)?;
            let mut user_splitted = user.split('_');
            let room_id = RoomId::from(
                user_splitted.next().ok_or(E::NoMemberId)?.to_string(),
            );
            let peer_id = PeerId(
                user_splitted
                    .next()
                    .ok_or(E::NoPeerId)?
                    .parse()
                    .map_err(E::FailedPeerIdParsing)?,
            );

            (room_id, peer_id)
        };

        let mut channel_splitted = channel_splitted.skip(1);

        let allocation_id: u64 = channel_splitted
            .next()
            .ok_or(E::NoAllocationId)?
            .parse()
            .map_err(E::FailedAllocationIdParsing)?;
        let event_type = channel_splitted.next().ok_or(E::NoEventType)?;

        let event = CoturnAllocationEvent::parse(
            event_type,
            msg.get_payload::<String>()
                .map_err(E::WrongPayloadType)?
                .as_str(),
        )?;

        Ok(CoturnEvent {
            event,
            room_id,
            peer_id,
            allocation_id,
        })
    }
}

/// Possible allocation events which can be thrown by Coturn.
#[derive(Debug)]
pub enum CoturnAllocationEvent {
    /// New allocation is created.
    New {
        /// Time for which this allocation will be available.
        ///
        /// If this time is changed by Coturn then
        /// [`CoturnAllocationEvent::Refreshed`] is sent.
        lifetime: Duration,
    },

    /// Allocation's lifetime is updated.
    Refreshed {
        /// New time for this this allocation will be available.
        ///
        /// If this time is `0` then allocation can be considered as deleted.
        lifetime: Duration,
    },

    /// Update of [`Traffic`] statistic.
    Traffic { traffic: Traffic },

    /// Total [`Traffic`] statistic of this allocation.
    ///
    /// Allocation can be considered as deleted.
    TotalTraffic { traffic: Traffic },

    /// Allocation is deleted.
    Deleted,
}

impl CoturnAllocationEvent {
    /// Tries to parse [`CoturnAllocationEvent`] from the provided `event_type`
    /// and `body`.
    ///
    /// `body` is interpreted differently based on the provided `event_type`.
    ///
    /// # Errors
    ///
    /// All errors from this function should never happen, so there is no sense
    /// to catch them individually.
    pub fn parse(
        event_type: &str,
        body: &str,
    ) -> Result<Self, CoturnEventParseError> {
        use CoturnEventParseError::{
            EmptyStatus, FailedLifetimeParsing, NoMetadataInStatus,
            UnsupportedEventType, UnsupportedStatus,
        };

        match event_type {
            "total_traffic" => Ok(CoturnAllocationEvent::TotalTraffic {
                traffic: Traffic::parse(body)?,
            }),
            "traffic" => Ok(CoturnAllocationEvent::Traffic {
                traffic: Traffic::parse(body)?,
            }),
            "status" => {
                let mut splitted = body.split(' ');
                let status = splitted.next().ok_or(EmptyStatus)?;
                match status {
                    "deleted" => Ok(CoturnAllocationEvent::Deleted),
                    "new" => {
                        let lifetime = splitted
                            .next()
                            .ok_or(NoMetadataInStatus)?
                            .replace("lifetime=", "")
                            .parse()
                            .map_err(FailedLifetimeParsing)?;
                        Ok(CoturnAllocationEvent::New {
                            lifetime: Duration::from_secs(lifetime),
                        })
                    }
                    "refreshed" => {
                        let lifetime = splitted
                            .next()
                            .ok_or(NoMetadataInStatus)?
                            .replace("lifetime=", "")
                            .parse()
                            .map_err(FailedLifetimeParsing)?;
                        Ok(CoturnAllocationEvent::Refreshed {
                            lifetime: Duration::from_secs(lifetime),
                        })
                    }
                    _ => Err(UnsupportedStatus(status.into())),
                }
            }
            _ => Err(UnsupportedEventType(event_type.into())),
        }
    }
}

/// Traffic stats of some allocation.
#[derive(Clone, Debug, Copy)]
pub struct Traffic {
    /// Count of packets received by allocation.
    pub received_packets: u64,

    /// Count of bytes received by allocation.
    pub received_bytes: u64,

    /// Count of packets sent by allocation.
    pub sent_packets: u64,

    /// Count of bytes sent by allocation.
    pub sent_bytes: u64,
}

impl Traffic {
    /// Tries to parse [`Traffic`] stats from the provided [`str`].
    ///
    /// # Errors
    ///
    /// All errors from this function should never happen, so there is no sense
    /// to catch them individually.
    pub fn parse(body: &str) -> Result<Self, CoturnEventParseError> {
        use CoturnEventParseError::{
            FailedToParseTrafficMap, FieldNotFoundInTrafficUpdate,
            IncorrectTrafficMapFormat,
        };

        let mut items: HashMap<&str, u64> = body
            .split(", ")
            .map(|i| {
                let mut splitted_item = i.split('=');
                let key = splitted_item
                    .next()
                    .ok_or_else(|| IncorrectTrafficMapFormat(body.into()))?;
                let value: u64 = splitted_item
                    .next()
                    .ok_or_else(|| IncorrectTrafficMapFormat(body.into()))?
                    .parse()
                    .map_err(|e| FailedToParseTrafficMap(body.into(), e))?;

                Ok((key, value))
            })
            .collect::<Result<_, _>>()?;

        let received_packets = items
            .remove("rcvp")
            .ok_or_else(|| FieldNotFoundInTrafficUpdate("rcvp".into()))?;
        let received_bytes = items
            .remove("rcvb")
            .ok_or_else(|| FieldNotFoundInTrafficUpdate("rcvb".into()))?;
        let sent_packets = items
            .remove("sentp")
            .ok_or_else(|| FieldNotFoundInTrafficUpdate("sentp".into()))?;
        let sent_bytes = items
            .remove("sentb")
            .ok_or_else(|| FieldNotFoundInTrafficUpdate("sentb".into()))?;

        Ok(Self {
            received_packets,
            received_bytes,
            sent_packets,
            sent_bytes,
        })
    }
}
