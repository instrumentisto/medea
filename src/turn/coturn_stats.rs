use std::{fmt, sync::Arc};

use async_trait::async_trait;
use derive_more::{Display, From};
use failure::Fail;
use rand::{distributions::Alphanumeric, Rng};

use crate::{
    api::control::{EndpointId, MemberId, RoomId},
    conf,
    log::prelude::*,
    media::IceUser,
    turn::repo::{TurnDatabase, TurnDatabaseErr},
};
use actix::{Actor, AsyncContext, StreamHandler, WrapFuture};
use futures::channel::mpsc;
use medea_client_api_proto::PeerId;
use medea_coturn_telnet::sessions_parser::TrafficUsage;
use redis::{ConnectionInfo, IntoConnectionInfo, PubSub};
use std::{collections::HashMap, time::Duration};

#[derive(Clone, Debug)]
pub struct Traffic {
    pub received_packets: u64,
    pub received_bytes: u64,
    pub sent_packets: u64,
    pub sent_bytes: u64,
}

impl Traffic {
    pub fn parse(body: String) -> Result<Self, CoturnEventParseError> {
        let mut items: HashMap<&str, u64> = body
            .split(", ")
            .map(|i| {
                let mut splitted_item = i.split('=');
                let key = splitted_item.next().ok_or_else(|| {
                    CoturnEventParseError::FailedToParseTrafficMap(body.clone())
                })?;
                let value: u64 = splitted_item
                    .next()
                    .ok_or_else(|| {
                        CoturnEventParseError::FailedToParseTrafficMap(
                            body.clone(),
                        )
                    })?
                    .parse()
                    .map_err(|_| {
                        CoturnEventParseError::FailedToParseTrafficMap(
                            body.clone(),
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
        body: String,
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
    event: CoturnAllocationEvent,
    room_id: RoomId,
    peer_id: PeerId,
    allocation_id: u64,
}

impl CoturnEvent {
    pub fn parse(msg: redis::Msg) -> Result<Self, CoturnEventParseError> {
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
            msg.get_payload().unwrap(),
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

pub struct CoturnStatsWatcher {
    client: redis::Client,
}

impl CoturnStatsWatcher {
    pub fn new<T: IntoConnectionInfo>(
        conf: T,
    ) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(conf)?;

        Ok(Self { client })
    }
}

impl Actor for CoturnStatsWatcher {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let mut conn = self.client.get_connection().unwrap();
        let (tx, rx) = mpsc::unbounded();
        std::thread::spawn(move || {
            let mut pubsub = conn.as_pubsub();
            pubsub
                .psubscribe("turn/realm/*/user/*/allocation/*")
                .unwrap();
            loop {
                let msg = pubsub.get_message().unwrap();
                if tx.unbounded_send(msg).is_err() {
                    break;
                }
            }
        });
        ctx.add_stream(rx);
    }
}

impl StreamHandler<redis::Msg> for CoturnStatsWatcher {
    fn handle(&mut self, item: redis::Msg, ctx: &mut Self::Context) {
        match CoturnEvent::parse(item) {
            Ok(event) => {
                debug!("Coturn stats: {:?}", event);
            }
            Err(e) => {
                error!("Coturn stats parse error: {:?}", e);
            }
        }
    }
}

pub fn new_coturn_stats_watcher(
    cf: &conf::Turn,
) -> Result<CoturnStatsWatcher, redis::RedisError> {
    let turn_db = CoturnStatsWatcher::new(ConnectionInfo {
        addr: Box::new(redis::ConnectionAddr::Tcp(
            cf.db.redis.ip.to_string(),
            cf.db.redis.port,
        )),
        db: cf.db.redis.db_number,
        passwd: if cf.db.redis.pass.is_empty() {
            None
        } else {
            Some(cf.db.redis.pass.clone())
        },
    })?;

    Ok(turn_db)
}
