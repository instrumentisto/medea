use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    time::{Duration, Instant},
};

use actix::{
    Actor, ActorFuture, Addr, AsyncContext, Handler, Message, StreamHandler,
    WrapFuture,
};
use derive_more::Display;
use futures::{channel::mpsc, StreamExt};
use medea_client_api_proto::PeerId;
use medea_coturn_telnet_client::sessions_parser::Session;
use patched_redis::ConnectionInfo;

use crate::{
    api::control::RoomId,
    conf,
    log::prelude::*,
    turn::cli::{CoturnCliError, CoturnTelnetClient},
};

#[derive(Clone, Debug, Copy)]
pub struct Traffic {
    pub received_packets: u64,
    pub received_bytes: u64,
    pub sent_packets: u64,
    pub sent_bytes: u64,
}

impl Traffic {
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

#[derive(Debug)]
pub enum CoturnAllocationEvent {
    New { lifetime: Duration },
    Refreshed { lifetime: Duration },
    Traffic { traffic: Traffic },
    TotalTraffic { traffic: Traffic },
    Deleted,
}

impl CoturnAllocationEvent {
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

#[derive(Debug)]
pub struct CoturnEvent {
    pub event: CoturnAllocationEvent,
    pub room_id: RoomId,
    pub peer_id: PeerId,
    pub allocation_id: u64,
}

impl CoturnEvent {
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
            let room_id = RoomId(
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

    /// No PeerId metadata.
    #[display(fmt = "No PeerId metadata.")]
    NoPeerId,

    /// No allocation ID metadata.
    #[display(fmt = "No allocation ID metadata.")]
    NoAllocationId,

    #[display(fmt = "No event type.")]
    NoEventType,
}
