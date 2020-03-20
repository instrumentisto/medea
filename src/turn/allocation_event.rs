use std::{collections::HashMap, time::Duration};

use derive_more::Display;
use medea_client_api_proto::PeerId;

use crate::api::control::RoomId;

/// Traffic stats of some allocation.
#[derive(Clone, Debug, Copy)]
pub struct Traffic {
    /// Count of packets received by allocation.
    pub received_packets: u64,

    /// Count of bytes received by allocation.
    pub received_bytes: u64,

    /// Count of packets received by allocation.
    pub sent_packets: u64,

    /// Count of bytes sent by allocation.
    pub sent_bytes: u64,
}

impl Traffic {
    /// Tries to parse [`Traffic`] stats from the provided [`str`].
    pub fn parse(body: &str) -> Result<Self, CoturnEventParseError> {
        let mut items: HashMap<&str, u64> = body
            .split(", ")
            .map(|i| {
                let mut splitted_item = i.split('=');
                let key = splitted_item.next().ok_or_else(|| {
                    CoturnEventParseError::FailedToParseTrafficMap(
                        body.to_string(),
                    )
                })?;
                let value: u64 = splitted_item
                    .next()
                    .ok_or_else(|| {
                        CoturnEventParseError::FailedToParseTrafficMap(
                            body.to_string(),
                        )
                    })?
                    .parse()
                    .map_err(|_| {
                        CoturnEventParseError::FailedToParseTrafficMap(
                            body.to_string(),
                        )
                    })?;

                Ok((key, value))
            })
            .collect::<Result<_, _>>()?;

        let received_packets = items.remove("rcvp").ok_or_else(|| {
            CoturnEventParseError::FieldNotFoundInTrafficUpdate(
                "rcvp".to_string(),
            )
        })?;
        let received_bytes = items.remove("rcvb").ok_or_else(|| {
            CoturnEventParseError::FieldNotFoundInTrafficUpdate(
                "rcvb".to_string(),
            )
        })?;
        let sent_packets = items.remove("sentp").ok_or_else(|| {
            CoturnEventParseError::FieldNotFoundInTrafficUpdate(
                "sentp".to_string(),
            )
        })?;
        let sent_bytes = items.remove("sentb").ok_or_else(|| {
            CoturnEventParseError::FieldNotFoundInTrafficUpdate(
                "sentb".to_string(),
            )
        })?;

        Ok(Self {
            received_packets,
            received_bytes,
            sent_packets,
            sent_bytes,
        })
    }
}

/// All type of the allocation events which can be thrown by Coturn.
#[derive(Debug)]
pub enum CoturnAllocationEvent {
    /// New allocation is created.
    New {
        /// Time for which this allocation will be available.
        ///
        /// This time may be changed by Coturn then
        /// [`CoturnAllocationEvent::Refreshed`] will be sent.
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
    /// `body` will be interpreted different based on provided `event_type`.
    pub fn parse(
        event_type: &str,
        body: &str,
    ) -> Result<Self, CoturnEventParseError> {
        match event_type {
            "total_traffic" => Ok(CoturnAllocationEvent::TotalTraffic {
                traffic: Traffic::parse(body)?,
            }),
            "traffic" => Ok(CoturnAllocationEvent::Traffic {
                traffic: Traffic::parse(body)?,
            }),
            "status" => {
                let mut splitted = body.split(' ');
                let status = splitted
                    .next()
                    .ok_or(CoturnEventParseError::EmptyStatus)?;
                match status {
                    "deleted" => Ok(CoturnAllocationEvent::Deleted),
                    "new" => {
                        let lifetime = splitted
                            .next()
                            .ok_or(CoturnEventParseError::NoMetadataInStatus)?
                            .replace("lifetime=", "")
                            .parse()
                            .map_err(|_| {
                                CoturnEventParseError::FailedLifetimeParsing
                            })?;
                        Ok(CoturnAllocationEvent::New {
                            lifetime: Duration::from_secs(lifetime),
                        })
                    }
                    "refreshed" => {
                        let lifetime = splitted
                            .next()
                            .ok_or(CoturnEventParseError::NoMetadataInStatus)?
                            .replace("lifetime=", "")
                            .parse()
                            .map_err(|_| {
                                CoturnEventParseError::FailedLifetimeParsing
                            })?;
                        Ok(CoturnAllocationEvent::Refreshed {
                            lifetime: Duration::from_secs(lifetime),
                        })
                    }
                    _ => Err(CoturnEventParseError::UnsupportedStatus(
                        status.to_string(),
                    )),
                }
            }
            _ => Err(CoturnEventParseError::UnsupportedEventType(
                event_type.to_string(),
            )),
        }
    }
}

/// Allocation event received from the Coturn.
#[derive(Debug)]
pub struct CoturnEvent {
    /// Actual allocation event received from the Coturn.
    pub event: CoturnAllocationEvent,

    /// [`RoomId`] for which this [`CoturnEvent`] was received.
    pub room_id: RoomId,

    /// [`PeerId`] for which this [`CoturnEvent`] was received.
    pub peer_id: PeerId,

    /// Allocation ID for which this [`CoturnEvent`] was received.
    pub allocation_id: u64,
}

impl CoturnEvent {
    /// Tries to parse [`CoturnEvnet]` from a provided Redis message.
    pub fn parse(
        msg: &patched_redis::Msg,
    ) -> Result<Self, CoturnEventParseError> {
        let channel: String = msg
            .get_channel()
            .map_err(|_| CoturnEventParseError::NoChannelInfo)?;
        let mut channel_splitted = channel.split('/').skip(4);

        let (room_id, peer_id) = {
            let user = channel_splitted
                .next()
                .ok_or(CoturnEventParseError::NoUserInfo)?;
            let mut user_splitted = user.split('_');
            let room_id = RoomId::from(
                user_splitted
                    .next()
                    .ok_or(CoturnEventParseError::NoMemberId)?
                    .to_string(),
            );
            let peer_id = PeerId(
                user_splitted
                    .next()
                    .ok_or(CoturnEventParseError::NoPeerId)?
                    .parse()
                    .map_err(|_| CoturnEventParseError::NoPeerId)?,
            );

            (room_id, peer_id)
        };

        let mut channel_splitted = channel_splitted.skip(1);

        let allocation_id: u64 = channel_splitted
            .next()
            .ok_or(CoturnEventParseError::NoAllocationId)?
            .parse()
            .map_err(|_| CoturnEventParseError::NoAllocationId)?;
        let event_type = channel_splitted
            .next()
            .ok_or(CoturnEventParseError::NoEventType)?;

        let event = CoturnAllocationEvent::parse(
            event_type,
            msg.get_payload::<String>().unwrap().as_str(),
        )?;

        Ok(CoturnEvent {
            event,
            room_id,
            peer_id,
            allocation_id,
        })
    }
}

/// Errors which can occur while [`CoturnEvent`] parsing.
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
    FieldNotFoundInTrafficUpdate(String),

    /// Failed to parse traffic event stat metadata.
    #[display(
        fmt = "Failed to parse traffic stat '{}' from traffic event.",
        _0
    )]
    FailedToParseTrafficMap(String),

    /// Status is empty.
    #[display(fmt = "Status is empty.")]
    EmptyStatus,

    /// Status doesn't have metadata.
    #[display(fmt = "Status doesn't have metadata.")]
    NoMetadataInStatus,

    /// Allocation lifetime parsing failed.
    #[display(fmt = "Allocation lifetime parsing failed.")]
    FailedLifetimeParsing,

    /// Redis channel info is empty.
    #[display(fmt = "Redis channel info is empty.")]
    NoChannelInfo,

    /// No user metadata.
    #[display(fmt = "No user metadata.")]
    NoUserInfo,

    /// No MemberId metadata.
    #[display(fmt = "No MemberId metadata.")]
    NoMemberId,

    /// No [`PeerId`] metadata.
    #[display(fmt = "No PeerId metadata.")]
    NoPeerId,

    /// No allocation ID metadata.
    #[display(fmt = "No allocation ID metadata.")]
    NoAllocationId,

    /// Event type is not provided.
    #[display(fmt = "No event type.")]
    NoEventType,
}
