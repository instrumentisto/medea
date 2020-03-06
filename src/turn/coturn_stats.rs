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
    signalling::{room::OnStartOnStopCallback, Room},
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
    event: CoturnAllocationEvent,
    room_id: RoomId,
    peer_id: PeerId,
    allocation_id: u64,
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

pub struct CoturnStatsWatcher;

pub async fn coturn_watcher_loop(
    client: patched_redis::Client,
) -> Result<(), patched_redis::RedisError> {
    let conn = client.get_async_connection().await?;
    let mut pusub = conn.into_pubsub();
    pusub.psubscribe("turn/realm/*/user/*/allocation/*").await?;
    let mut pubsub_stream = pusub.on_message();

    while let Some(msg) = pubsub_stream.next().await {
        match CoturnEvent::parse(&msg) {
            Ok(event) => {
                debug!("Coturn stats: {:?}", event);
            }
            Err(e) => {
                error!("Coturn stats parse error: {:?}", e);
            }
        }
    }

    Ok(())
}

pub fn run_coturn_stats_watcher(cf: &conf::Turn) {
    let connection_info = ConnectionInfo {
        addr: Box::new(patched_redis::ConnectionAddr::Tcp(
            cf.db.redis.host.to_string(),
            cf.db.redis.port,
        )),
        db: cf.db.redis.db_number,
        passwd: if cf.db.redis.pass.is_empty() {
            None
        } else {
            Some(cf.db.redis.pass.to_string())
        },
    };
    let client = patched_redis::Client::open(connection_info).unwrap();
    actix::spawn(async move {
        coturn_watcher_loop(client).await.ok();
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublisherAllocationState {
    Stopped,
    Playing,
}

// TODO: when this will be Dropped - spawn on_stop event (if state is not
// PublisherAllocationState::Stopped).
#[derive(Debug)]
pub struct PublisherAllocation {
    allocation_id: u64,
    peer_id: PeerId,
    last_update: Instant,
    prev_sent_bytes: u64,
    prev_received_bytes: u64,
    current_sent_bytes: u64,
    current_received_bytes: u64,
    state: PublisherAllocationState,
    // TODO: recipient
    room_addr: Addr<Room>,
    events: HashSet<EventType>,
}

impl PublisherAllocation {
    pub fn update_traffic(&mut self, traffic: Traffic) {
        self.update_send_bytes(traffic.sent_bytes);
        self.update_received_bytes(traffic.received_bytes);
    }

    fn update_send_bytes(&mut self, sent_bytes: u64) {
        if self.current_sent_bytes < sent_bytes {
            self.prev_sent_bytes = self.current_sent_bytes;
            self.current_sent_bytes = sent_bytes;
        }
        self.refresh_last_update();
    }

    fn update_received_bytes(&mut self, received_bytes: u64) {
        if self.current_received_bytes < received_bytes {
            self.prev_received_bytes = self.current_received_bytes;
            self.current_received_bytes = received_bytes;
        }
        self.refresh_last_update();
    }

    fn refresh_last_update(&mut self) {
        self.last_update = Instant::now();
    }

    pub fn on_start(&mut self) {
        self.state = PublisherAllocationState::Playing;
        self.room_addr.do_send(OnStartOnStopCallback {
            peer_id: self.peer_id,
            event: EventType::OnStart,
        });
    }

    pub fn on_stop(&mut self) {
        self.state = PublisherAllocationState::Stopped;

        self.room_addr.do_send(OnStartOnStopCallback {
            peer_id: self.peer_id,
            event: EventType::OnStop,
        });
    }
}

impl Drop for PublisherAllocation {
    fn drop(&mut self) {
        self.room_addr.do_send(OnStartOnStopCallback {
            peer_id: self.peer_id,
            event: EventType::OnStop,
        })
    }
}

#[derive(Eq, PartialEq, Hash, Debug)]
pub enum EventType {
    OnStart,
    OnStop,
}

type Alloc = Rc<RefCell<PublisherAllocation>>;

#[derive(Debug)]
pub struct CoturnStats {
    allocations: HashMap<u64, (Alloc, Option<Alloc>)>,
    client: patched_redis::Client,
    awaits_allocation: HashMap<PeerId, Subscribe>,
    coturn_client: CoturnTelnetClient,
    relay_allocations_ids: HashMap<PeerId, u64>,
}

impl CoturnStats {
    pub fn new(
        cf: &crate::conf::turn::Turn,
        coturn_client: CoturnTelnetClient,
    ) -> Result<Self, patched_redis::RedisError> {
        let connection_info = ConnectionInfo {
            addr: Box::new(patched_redis::ConnectionAddr::Tcp(
                cf.db.redis.host.to_string(),
                cf.db.redis.port,
            )),
            db: cf.db.redis.db_number,
            passwd: if cf.db.redis.pass.is_empty() {
                None
            } else {
                Some(cf.db.redis.pass.to_string())
            },
        };
        let client = patched_redis::Client::open(connection_info)?;

        Ok(Self {
            allocations: HashMap::new(),
            client,
            awaits_allocation: HashMap::new(),
            coturn_client,
            relay_allocations_ids: HashMap::new(),
        })
    }

    pub fn try_init_allocation(
        &mut self,
        sessions: Vec<Session>,
        peer_id: PeerId,
    ) -> Result<(), CoturnCliError> {
        let relay_session =
            if let Some(relay_session) = find_relay_session(sessions) {
                relay_session
            } else {
                return Ok(());
            };
        let subscription_req =
            if let Some(s) = self.awaits_allocation.remove(&peer_id) {
                s
            } else {
                return Ok(());
            };
        let partner_peer_id = subscription_req.partner_peer_id;
        let allocation = Rc::new(RefCell::new(PublisherAllocation {
            allocation_id: relay_session.id.0,
            peer_id,
            state: PublisherAllocationState::Playing,
            current_received_bytes: relay_session.traffic_usage.received_bytes,
            current_sent_bytes: relay_session.traffic_usage.sent_bytes,
            prev_received_bytes: 0,
            prev_sent_bytes: 0,
            events: subscription_req.events_type,
            last_update: Instant::now(),
            room_addr: subscription_req.addr,
        }));
        allocation.borrow_mut().on_start();

        if let Some(partner_allocation_id) =
            self.relay_allocations_ids.get(&partner_peer_id)
        {
            let partner_allocation_subs = self
                .allocations
                .get_mut(partner_allocation_id)
                .expect("476");
            let partner_allocation = partner_allocation_subs.0.clone();
            partner_allocation_subs.1 = Some(allocation.clone());

            self.allocations.insert(
                relay_session.id.0,
                (allocation, Some(partner_allocation)),
            );
        } else {
            self.allocations
                .insert(relay_session.id.0, (allocation, None));
        }
        self.relay_allocations_ids
            .insert(peer_id, relay_session.id.0);

        Ok(())
    }
}

impl Actor for CoturnStats {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let (msg_tx, msg_stream) = mpsc::unbounded();
        let client = self.client.clone();

        ctx.spawn(
            async move {
                let conn = client.get_async_connection().await.unwrap();
                let mut pubsub = conn.into_pubsub();
                pubsub
                    .psubscribe("turn/realm/*/user/*/allocation/*")
                    .await
                    .unwrap();

                let mut msg_stream = pubsub.on_message();
                while msg_tx.unbounded_send(msg_stream.next().await).is_ok() {}
            }
            .into_actor(self),
        );

        ctx.run_interval(Duration::from_millis(500), |this, _| {
            // TODO: use partner allocation
            for (allocation, _partner_allocation) in this.allocations.values() {
                let last_update = allocation.borrow().last_update;
                let allocation_state = allocation.borrow().state;
                if last_update + Duration::from_secs(3) < Instant::now() {
                    if allocation_state == PublisherAllocationState::Playing {
                        allocation.borrow_mut().on_stop();
                    }
                } else if allocation_state == PublisherAllocationState::Stopped
                {
                    allocation.borrow_mut().on_start();
                }
            }
        });

        ctx.add_stream(msg_stream);
    }
}

fn find_relay_session(sessions: Vec<Session>) -> Option<Session> {
    for session in sessions {
        if !session.peers.is_empty() {
            return Some(session);
        }
    }

    None
}

impl StreamHandler<Option<patched_redis::Msg>> for CoturnStats {
    fn handle(
        &mut self,
        item: Option<patched_redis::Msg>,
        ctx: &mut Self::Context,
    ) {
        if let Some(msg) = item {
            let event = if let Ok(event) = CoturnEvent::parse(&msg) {
                event
            } else {
                return;
            };
            let peer_id = event.peer_id;

            match event.event {
                CoturnAllocationEvent::Traffic { traffic } => {
                    if let Some((allocation, partner_allocation)) =
                        self.allocations.get(&event.allocation_id)
                    {
                        allocation.borrow_mut().update_traffic(traffic);
                        if let Some(partner_allocation) = partner_allocation {
                            partner_allocation
                                .borrow_mut()
                                .update_received_bytes(traffic.sent_bytes);
                            partner_allocation
                                .borrow_mut()
                                .update_send_bytes(traffic.received_bytes);
                        }
                    } else if let Some(subscription_request) =
                        self.awaits_allocation.get(&event.peer_id)
                    {
                        let coturn_client = self.coturn_client.clone();
                        let room_id = subscription_request.room_id.clone();
                        ctx.spawn(
                            async move {
                                let sess = format!(
                                    "{}_{}",
                                    room_id,
                                    event.peer_id.to_string()
                                );
                                coturn_client.get_sessions(sess).await
                            }
                            .into_actor(self)
                            .map(
                                move |res, this, _| match res {
                                    Ok(sessions) => {
                                        if let Err(e) = this
                                            .try_init_allocation(
                                                sessions, peer_id,
                                            )
                                        {
                                            error!(
                                                "Allocation init error: {:?}",
                                                e
                                            );
                                        };
                                    }
                                    Err(e) => {
                                        error!("Coturn CLI error: {:?}", e);
                                    }
                                },
                            ),
                        );
                    }
                }
                CoturnAllocationEvent::Deleted => {
                    if let Some((_, partner_allocation)) =
                        self.allocations.remove(&event.allocation_id)
                    {
                        if let Some(partner_allocation) = partner_allocation {
                            let partner_allocation_id =
                                partner_allocation.borrow().allocation_id;
                            self.allocations.remove(&partner_allocation_id);
                        }
                    }
                }
                _ => (),
            }
        } else {
            todo!("Implement reconnection logic.")
        }
    }
}

#[derive(Message, Debug)]
#[rtype(result = "Result<(), CoturnCliError>")]
pub struct Subscribe {
    // TODO: recipient
    pub addr: Addr<Room>,
    pub events_type: HashSet<EventType>,
    pub room_id: RoomId,
    pub partner_peer_id: PeerId,
    pub peer_id: PeerId,
}

pub type ActFuture<O> = Box<dyn ActorFuture<Actor = CoturnStats, Output = O>>;

impl Handler<Subscribe> for CoturnStats {
    type Result = ActFuture<Result<(), CoturnCliError>>;

    fn handle(
        &mut self,
        msg: Subscribe,
        _: &mut Self::Context,
    ) -> Self::Result {
        let coturn_client = self.coturn_client.clone();
        let peer_id = msg.peer_id;
        let session_id = format!("{}_{}", msg.room_id, msg.peer_id);
        self.awaits_allocation.insert(peer_id, msg);
        Box::new(
            async move { coturn_client.get_sessions(session_id).await }
                .into_actor(self)
                .map(move |res, this, _| {
                    match res {
                        Ok(sessions) => {
                            if let Err(e) =
                                this.try_init_allocation(sessions, peer_id)
                            {
                                error!("Allocation init failed: {:?}", e);
                            };
                        }
                        Err(e) => {
                            error!("Get Coturn sessions failed: {:?}", e);
                        }
                    }

                    Ok(())
                }),
        )
    }
}
