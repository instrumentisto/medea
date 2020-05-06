//! Service which is responsible for processing [`Peer`]s [`RtcStat`] metrics.
//!
//! At first you must register Peer via [`PeersMetricsService.register_peer()`].
//! Use [`PeersMetricsService.subscribe()`] to subscribe to stats processing
//! results. Then provide Peer metrics to [`PeersMetricsService.add_stat()`].
//! You should call [`PeersMetricsService.check_peers()`] with
// reasonable interval (~1-2 sec), this will check for stale metrics.
//! This service acts as flow and stop metrics source for the
//! [`PeerTrafficWatcher`].

// TODO: remove in #91
#![allow(dead_code)]

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use futures::{channel::mpsc, Stream};
use medea_client_api_proto::{
    stats::{
        RtcInboundRtpStreamMediaType, RtcInboundRtpStreamStats,
        RtcOutboundRtpStreamMediaType, RtcOutboundRtpStreamStats, RtcStat,
        RtcStatsType, StatId,
    },
    Direction, MediaType as MediaTypeProto, PeerId,
};
use medea_macro::dispatchable;

use crate::{
    api::control::{
        callback::{MediaDirection, MediaType},
        RoomId,
    },
    log::prelude::*,
    media::PeerStateMachine as Peer,
    signalling::peers::FlowMetricSource,
    utils::instant_into_utc,
};

use super::traffic_watcher::PeerTrafficWatcher;
use crate::{
    conf::Media,
    signalling::peers::media_traffic_state::{
        which_media_type_was_started, which_media_type_was_stopped,
        MediaTrafficState,
    },
};

/// Service which is responsible for processing [`Peer`]s [`RtcStat`] metrics.
#[derive(Debug)]
pub struct PeersMetricsService {
    /// [`RoomId`] of Room to which this [`PeersMetricsService`] belongs
    /// to.
    room_id: RoomId,

    /// [`Addr`] of [`PeerTrafficWatcher`] to which traffic updates will be
    /// sent.
    peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,

    /// All `PeerConnection` for this [`PeersMetricsService`] will process
    /// metrics.
    peers: HashMap<PeerId, Rc<RefCell<PeerStat>>>,

    /// Sender of [`PeerMetricsEvent`]s.
    ///
    /// Currently [`PeerMetricsEvent`] will receive [`Room`] to which this
    /// [`PeersMetricsService`] belongs to.
    events_tx: Option<mpsc::UnboundedSender<PeersMetricsEvent>>,
}

impl PeersMetricsService {
    /// Returns new [`PeersMetricsService`] for a provided [`Room`].
    pub fn new(
        room_id: RoomId,
        peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,
    ) -> Self {
        Self {
            room_id,
            peers_traffic_watcher,
            peers: HashMap::new(),
            events_tx: None,
        }
    }

    /// Returns [`Stream`] of [`PeerMetricsEvent`]s.
    ///
    /// Creating new subscription will invalidate previous, so there may be only
    /// one subscription. Events are not saved or buffered at sending side, so
    /// you won't receive any events happened before subscription was made.
    pub fn subscribe(&mut self) -> impl Stream<Item = PeersMetricsEvent> {
        let (tx, rx) = mpsc::unbounded();
        self.events_tx = Some(tx);

        rx
    }

    /// Checks that all tracked [`Peer`]s are valid, meaning that all their
    /// inbound and outbound tracks are flowing according to the provided
    /// metrics.
    ///
    /// Sends [`PeersMetricsEvent::NoTrafficFlow`] message if it determines that
    /// some track is not flowing.
    pub fn check_peers(&mut self) {
        let mut stopped_peers = Vec::new();
        for peer in self
            .peers
            .values()
            .filter(|peer| peer.borrow().state == PeerStatState::Connected)
        {
            let mut peer_ref = peer.borrow_mut();
            let send_media_traffic_state_before = peer_ref.send_traffic_state;
            let recv_media_traffic_state_before = peer_ref.recv_traffic_state;
            peer_ref.update_media_traffic_state();

            if peer_ref.is_stopped() {
                debug!(
                    "Peer [id = {}] from Room [id = {}] traffic stopped \
                     because all his traffic not flowing.",
                    peer_ref.peer_id, self.room_id
                );
                self.peers_traffic_watcher.traffic_stopped(
                    self.room_id.clone(),
                    peer_ref.peer_id,
                    peer_ref.get_stop_time(),
                );
                stopped_peers.push(peer_ref.peer_id);
            } else {
                let send_media_traffic_state_after =
                    peer_ref.send_traffic_state;
                let recv_media_traffic_state_after =
                    peer_ref.recv_traffic_state;
                if let Some(stopped_send_media_type) =
                which_media_type_was_stopped(
                    send_media_traffic_state_before,
                    send_media_traffic_state_after,
                )
                {
                    let stopped_at = peer_ref.get_tracks_last_update(
                        MediaDirection::Publish,
                        stopped_send_media_type,
                    );
                    self.wrong_traffic_flowing(
                        peer_ref.peer_id,
                        convert_instant_to_utc(stopped_at),
                        stopped_send_media_type,
                        MediaDirection::Publish,
                    );
                }
                if let Some(stopped_recv_media_type) =
                which_media_type_was_stopped(
                    recv_media_traffic_state_before,
                    recv_media_traffic_state_after,
                )
                {
                    let stopped_at = peer_ref.get_tracks_last_update(
                        MediaDirection::Publish,
                        stopped_recv_media_type,
                    );
                    self.wrong_traffic_flowing(
                        peer_ref.peer_id,
                        convert_instant_to_utc(stopped_at),
                        stopped_recv_media_type,
                        MediaDirection::Publish,
                    );
                }
            }
        }

        for stopped_peer_id in stopped_peers {
            self.peers.remove(&stopped_peer_id);
        }
    }

    /// [`Room`] notifies [`PeersMetricsService`] about new `PeerConnection`s
    /// creation.
    ///
    /// Based on the provided [`PeerSpec`]s [`PeerStat`]s will be validated.
    pub fn register_peer(&mut self, peer: &Peer, stats_ttl: Duration) {
        debug!(
            "Peer [id = {}] was registered in the PeerMetricsService [room_id \
             = {}].",
            peer.id(),
            self.room_id
        );

        let first_peer_stat = Rc::new(RefCell::new(PeerStat {
            peer_id: peer.id(),
            partner_peer: Weak::new(),
            last_update: Utc::now(),
            senders: HashMap::new(),
            receivers: HashMap::new(),
            send_traffic_state: MediaTrafficState::new(),
            recv_traffic_state: MediaTrafficState::new(),
            state: PeerStatState::Connecting,
            spec: PeerTracks::from(peer),
            peer_validity_timeout,
        }));
        if let Some(partner_peer_stat) = self.peers.get(&peer.partner_peer_id())
        {
            first_peer_stat.borrow_mut().partner_peer =
                Rc::downgrade(&partner_peer_stat);
            partner_peer_stat.borrow_mut().partner_peer =
                Rc::downgrade(&first_peer_stat);
        }

        self.peers.insert(peer.id(), first_peer_stat);
    }

    /// Adds new [`RtcStat`]s for the [`PeerStat`]s from this
    /// [`PeersMetricsService`].
    pub fn add_stat(&mut self, peer_id: PeerId, stats: Vec<RtcStat>) {
        if let Some(peer) = self.peers.get(&peer_id) {
            let mut peer_ref = peer.borrow_mut();
            let send_media_traffic_state_before = peer_ref.send_traffic_state;
            let recv_media_traffic_state_before = peer_ref.recv_traffic_state;
            peer_ref.update_media_traffic_state();

            for stat in stats {
                match &stat.stats {
                    RtcStatsType::InboundRtp(inbound) => {
                        peer_ref.update_receiver(stat.id, inbound);
                    }
                    RtcStatsType::OutboundRtp(outbound) => {
                        peer_ref.update_sender(stat.id, outbound);
                    }
                    _ => (),
                }
            }

            if peer_ref.is_stopped() {
                debug!(
                    "Peer [id = {}] from Room [id = {}] traffic stopped \
                     because traffic stats doesn't updated too long.",
                    peer_ref.peer_id, self.room_id
                );
                self.peers_traffic_watcher.traffic_stopped(
                    self.room_id.clone(),
                    peer_ref.peer_id,
                    peer_ref.get_stop_time(),
                );
            } else {
                self.peers_traffic_watcher.traffic_flows(
                    self.room_id.clone(),
                    peer_id,
                    FlowMetricSource::Peer,
                );
                if let Some(partner_peer_id) = peer_ref.get_partner_peer_id() {
                    self.peers_traffic_watcher.traffic_flows(
                        self.room_id.clone(),
                        partner_peer_id,
                        FlowMetricSource::PartnerPeer,
                    );
                }

                let send_media_traffic_state_after =
                    peer_ref.send_traffic_state;
                let recv_media_traffic_state_after =
                    peer_ref.recv_traffic_state;

                if let Some(started_media_type) = which_media_type_was_started(
                    send_media_traffic_state_before,
                    send_media_traffic_state_after,
                ) {
                    self.track_traffic_starts_flowing(
                        peer_id,
                        started_media_type,
                        MediaDirection::Publish,
                    );
                }

                if let Some(started_media_type) = which_media_type_was_started(
                    recv_media_traffic_state_before,
                    recv_media_traffic_state_after,
                ) {
                    self.track_traffic_starts_flowing(
                        peer_id,
                        started_media_type,
                        MediaDirection::Play,
                    );
                }

                if let Some(stopped_media_type) = which_media_type_was_stopped(
                    send_media_traffic_state_before,
                    send_media_traffic_state_after,
                ) {
                    let stopped_at = peer_ref.get_tracks_last_update(
                        MediaDirection::Publish,
                        stopped_media_type,
                    );
                    self.wrong_traffic_flowing(
                        peer_id,
                        convert_instant_to_utc(stopped_at),
                        stopped_media_type,
                        MediaDirection::Publish,
                    );
                }

                if let Some(stopped_media_type) = which_media_type_was_stopped(
                    recv_media_traffic_state_before,
                    recv_media_traffic_state_after,
                ) {
                    let stopped_at = peer_ref.get_tracks_last_update(
                        MediaDirection::Play,
                        stopped_media_type,
                    );
                    self.wrong_traffic_flowing(
                        peer_id,
                        convert_instant_to_utc(stopped_at),
                        stopped_media_type,
                        MediaDirection::Play,
                    );
                }
            }
        }
    }

    /// Stops tracking provided [`Peer`]s.
    pub fn unregister_peers(&mut self, peers_ids: &[PeerId]) {
        debug!(
            "Peers [ids = [{:?}]] from Room [id = {}] was unsubscribed from \
             the PeerMetricsService.",
            peers_ids, self.room_id
        );

        for peer_id in peers_ids {
            self.peers.remove(peer_id);
        }
    }

    /// Updates [`Peer`]s internal representation. Must be called each time
    /// [`Peer`] tracks set changes (some track was added or removed).
    pub fn update_peer_spec(&mut self, peer: &Peer) {
        if let Some(peer_stat) = self.peers.get(&peer.id()) {
            peer_stat.borrow_mut().tracks_spec = PeerTracks::from(peer);
        }
    }

    /// Sends [`PeerMetricsEvent::NoTrafficFlow`] event to subscriber.
    fn send_no_traffic(
        &self,
        peer_id: PeerId,
        was_flowing_at: DateTime<Utc>,
        media_type: MediaType,
        direction: MediaDirection,
    ) {
        if let Some(sender) = &self.events_tx {
            let _ = sender.unbounded_send(PeersMetricsEvent::NoTrafficFlow {
                peer_id,
                was_flowing_at,
                media_type,
                direction,
            });
        }
    }

    /// Stopped `MediaTrack` with provided [`MediaType`] and [`MediaDirection`]
    /// was started after stopping.
    fn track_traffic_starts_flowing(
        &self,
        peer_id: PeerId,
        media_type: MediaType,
        direction: MediaDirection,
    ) {
        if let Some(sender) = &self.events_tx {
            let _ = sender.unbounded_send(PeersMetricsEvent::TrafficFlows {
                peer_id,
                media_type,
                direction,
            });
        }
    }
}

/// Media type of a [`MediaTrack`].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq, Hash)]
enum TrackMediaType {
    Audio,
    Video,
}

impl From<TrackMediaType> for MediaType {
    fn from(from: TrackMediaType) -> Self {
        match from {
            TrackMediaType::Audio => Self::Audio,
            TrackMediaType::Video => Self::Video,
        }
    }
}

impl PartialEq<MediaType> for TrackMediaType {
    fn eq(&self, other: &MediaType) -> bool {
        match other {
            MediaType::Video => *self == MediaType::Video,
            MediaType::Audio => *self == MediaType::Audio,
            MediaType::Both => true,
        }
    }
}

/// Events which [`PeersMetricsService`] can send to its subscriber.
#[dispatchable]
#[derive(Debug, Clone)]
pub enum PeersMetricsEvent {
    /// Wrong [`Peer`] traffic flowing was detected. Some `MediaTrack`s with
    /// provided [`TrackMediaType`] doesn't flows.
    NoTrafficFlow {
        peer_id: PeerId,
        was_flowing_at: DateTime<Utc>,
        media_type: MediaType,
        direction: MediaDirection,
    },

    /// Stopped `MediaTrack` with provided [`MediaType`] and [`MediaDirection`]
    /// was started after stopping.
    TrafficFlows {
        peer_id: PeerId,
        media_type: MediaType,
        direction: MediaDirection,
    },
}

/// Specification of a [`Peer`]s tracks. Contains info about how many tracks of
/// each kind should this [`Peer`] send/receive.
///
/// This spec is compared with [`Peer`]s actual stats, to calculate difference
/// between expected and actual [`Peer`] state.
#[derive(Debug)]
struct PeerTracks {
    audio_send: u64,
    video_send: u64,
    audio_recv: u64,
    video_recv: u64,
}

impl From<&Peer> for PeerTracks {
    fn from(peer: &Peer) -> Self {
        // TODO: filter muted MediaTracks.
        let mut audio_send = 0;
        let mut video_send = 0;
        let mut audio_recv = 0;
        let mut video_recv = 0;

        for sender in peer.senders().values() {
            match sender.media_type {
                MediaTypeProto::Audio(_) => audio_send += 1,
                MediaTypeProto::Video(_) => video_send += 1,
            }
        }
        for receiver in peer.receivers().values() {
            match receiver.media_type {
                MediaTypeProto::Audio(_) => audio_recv += 1,
                MediaTypeProto::Video(_) => video_recv += 1,
            }
        }

        Self {
            audio_send,
            video_send,
            audio_recv,
            video_recv,
        }
    }
}

/// Metrics which are available for `MediaTrack` with `Send` direction.
#[derive(Debug)]
struct Send {
    /// Count of packets sent by a `MediaTrack` which this [`TrackStat`]
    /// represents.
    packets_sent: u64,
}

/// Metrics which are available for `MediaTrack` with `Recv` direction.
#[derive(Debug)]
struct Recv {
    /// Count of packets received by a `MediaTrack` which this [`TrackStat`]
    /// represents.
    packets_received: u64,
}

/// Metrics of the `MediaTrack` with [`SendDir`] or [`RecvDir`] state.
#[derive(Debug)]
struct TrackStat<T> {
    /// Last time when this [`TrackStat`] was updated.
    updated_at: Instant,

    /// Media type of the `MediaTrack` which this [`TrackStat`] represents.
    media_type: TrackMediaType,

    /// Direction state of this [`TrackStat`].
    ///
    /// Can be [`SendDir`] or [`RecvDir`].
    direction: T,
}

impl<T> TrackStat<T> {
    /// Returns [`Instant`] time on which this [`TrackStat`] was updated last
    /// time.
    fn updated_at(&self) -> &Instant {
        &self.updated_at
    }
}

impl TrackStat<Send> {
    /// Updates this [`TrackStat`] with provided [`RtcOutboundRtpStreamStats`].
    ///
    /// [`TrackStat::last_update`] time will be updated.
    fn update(&mut self, upd: &RtcOutboundRtpStreamStats) {
        self.updated_at = Instant::now();
        self.direction.packets_sent = upd.packets_sent;
    }
}

impl TrackStat<Recv> {
    /// Updates this [`TrackStat`] with provided [`RtcInboundRtpStreamStats`].
    ///
    /// [`TrackStat::last_update`] time will be updated.
    fn update(&mut self, upd: &RtcInboundRtpStreamStats) {
        self.updated_at = Instant::now();
        self.direction.packets_received = upd.packets_received;
    }
}

/// Current state of a [`PeerStat`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeerStatState {
    /// [`Peer`] which this [`PeerStat`] represents is considered as
    /// connected.
    Connected,

    /// [`Peer`] which this [`PeerStat`] represents waiting for
    /// connection.
    Connecting,
}

/// Current stats of some [`Peer`].
#[derive(Debug)]
struct PeerStat {
    /// [`PeerId`] of [`Peer`] which this [`PeerStat`] represents.
    peer_id: PeerId,

    /// Weak reference to a [`PeerStat`] which represents a partner
    /// [`Peer`].
    partner_peer: Weak<RefCell<PeerStat>>,

    /// Specification of a [`Peer`] which this [`PeerStat`] represents.
    tracks_spec: PeerTracks,

    /// All [`TrackStat`]s with [`Send`] direction of this [`PeerStat`].
    senders: HashMap<StatId, TrackStat<Send>>,

    send_traffic_state: MediaTrafficState,

    recv_traffic_state: MediaTrafficState,

    /// All [`TrackStat`]s with [`Recv`] of this [`PeerStat`].
    receivers: HashMap<StatId, TrackStat<Recv>>,

    /// Current connection state of this [`PeerStat`].
    state: PeerStatState,

    /// Time of the last metrics update of this [`PeerStat`].
    last_update: DateTime<Utc>,

    /// Duration, after which [`Peers`] stats will be considered as stale.
    stats_ttl: Duration,
}

impl PeerStat {
    /// Updates [`TrackStat`] with provided [`StatId`] by
    /// [`RtcOutboundRtpStreamStats`].
    fn update_sender(
        &mut self,
        stat_id: StatId,
        upd: &RtcOutboundRtpStreamStats,
    ) {
        self.last_update = Utc::now();
        self.senders
            .entry(stat_id.clone())
            .or_insert_with(|| TrackStat {
                updated_at: Instant::now(),
                direction: Send { packets_sent: 0 },
                media_type: TrackMediaType::from(&upd.media_type),
            })
            .update(upd);
        let sender = self.senders.get(&stat_id).unwrap();
        if self.is_track_active(&sender) {
            let sender_media_type: MediaType = sender.media_type.into();
            self.send_traffic_state.started(sender_media_type);
        }
    }

    /// Updates [`TrackStat`] with provided [`StatId`] by
    /// [`RtcInboundRtpStreamStats`].
    fn update_receiver(
        &mut self,
        stat_id: StatId,
        upd: &RtcInboundRtpStreamStats,
    ) {
        self.last_update = Utc::now();
        self.receivers
            .entry(stat_id.clone())
            .or_insert_with(|| TrackStat {
                updated_at: Instant::now(),
                direction: Recv {
                    packets_received: 0,
                },
                media_type: TrackMediaType::from(&upd.media_specific_stats),
            })
            .update(upd);
        let receiver = self.receivers.get(&stat_id).unwrap();
        if self.is_track_active(&receiver) {
            let receiver_media_type = receiver.media_type.into();
            self.recv_traffic_state.started(receiver_media_type);
        }
    }

    /// Returns last update time of the tracks with provided [`MediaDirection`]
    /// and [`MediaType`].
    #[allow(clippy::filter_map)]
    fn get_tracks_last_update(
        &self,
        direction: MediaDirection,
        media_type: MediaType,
    ) -> Instant {
        match direction {
            MediaDirection::Play => self
                .receivers
                .values()
                .filter(|recv| recv.media_type == media_type)
                .map(|recv| recv.updated_at)
                .max()
                .unwrap_or_else(Instant::now),
            MediaDirection::Publish => self
                .senders
                .values()
                .filter(|send| send.media_type == media_type)
                .map(|send| send.updated_at)
                .max()
                .unwrap_or_else(Instant::now),
        }
    }

    /// Checks that media traffic flows through provided [`TrackStat`].
    ///
    /// [`TrackStat`] should be updated within
    /// [`PeerStat::stats_ttl`] or this [`TrackStat`] will be
    /// considered as stopped.
    fn is_track_active<T>(&self, track: &TrackStat<T>) -> bool {
        track.updated_at().elapsed() < self.stats_ttl
    }

    /// Returns [`MediaDirection`]s and [`MediaType`]s of the `MediaTrack`s
    /// which are currently is stopped.
    ///
    /// This is determined by comparing count of senders/receivers from the
    /// [`PeerSpec`].
    ///
    /// Also media type of sender/receiver
    /// and activity taken into account.
    fn update_media_traffic_state(&mut self) {
        let mut audio_send = 0;
        let mut video_send = 0;
        let mut audio_recv = 0;
        let mut video_recv = 0;

        self.senders
            .values()
            .filter(|t| self.is_track_active(&t))
            .for_each(|sender| match sender.media_type {
                TrackMediaType::Audio => audio_send += 1,
                TrackMediaType::Video => video_send += 1,
            });
        self.receivers
            .values()
            .filter(|t| self.is_track_active(&t))
            .for_each(|receiver| match receiver.media_type {
                TrackMediaType::Audio => audio_recv += 1,
                TrackMediaType::Video => video_recv += 1,
            });

        if audio_send < self.spec.audio_send {
            self.send_traffic_state.stopped(MediaType::Audio);
        } else {
            self.send_traffic_state.started(MediaType::Audio);
        }
        if video_send < self.spec.video_send {
            self.send_traffic_state.stopped(MediaType::Video);
        } else {
            self.send_traffic_state.started(MediaType::Video);
        }
        if audio_recv < self.spec.audio_recv {
            self.recv_traffic_state.stopped(MediaType::Audio);
        } else {
            self.recv_traffic_state.started(MediaType::Audio);
        }
        if video_recv < self.spec.video_recv {
            self.recv_traffic_state.stopped(MediaType::Video);
        } else {
            self.recv_traffic_state.started(MediaType::Video);
        }
    }

    /// Returns `true` if all senders and receivers is not sending or receiving
    /// anything.
    fn is_stopped(&self) -> bool {
        self.recv_traffic_state.is_stopped(MediaType::Both)
            && self.send_traffic_state.is_stopped(MediaType::Both)

        // let active_senders_count = self
        //     .senders
        //     .values()
        //     .filter(|sender| self.is_track_active(&sender))
        //     .count();
        // let active_receivers_count = self
        //     .receivers
        //     .values()
        //     .filter(|recv| self.is_track_active(&recv))
        //     .count();
        //
        // active_receivers_count + active_senders_count == 0
    }

    /// Returns [`Instant`] time of [`TrackStat`] which haven't updated longest.
    fn get_stop_time(&self) -> Instant {
        self.senders
            .values()
            .map(|send| send.updated_at)
            .chain(self.receivers.values().map(|recv| recv.updated_at))
            .min()
            .unwrap_or_else(Instant::now)
    }

    /// Returns `Some` [`PeerId`] of a partner [`Peer`] if partner
    /// [`PeerStat`]'s weak pointer is available.
    ///
    /// Returns `None` if weak pointer of partner [`PeerStat`] is unavailable.
    fn get_partner_peer_id(&self) -> Option<PeerId> {
        self.partner_peer
            .upgrade()
            .map(|partner_peer| partner_peer.borrow().get_peer_id())
    }

    /// Returns [`PeerId`] of [`Peer`] which this [`PeerStat`]
    /// represents.
    fn get_peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Sets state of this [`PeerStat`] to [`PeerStatState::Connected`].
    fn connected(&mut self) {
        self.state = PeerStatState::Connected;
    }
}

impl From<&RtcOutboundRtpStreamMediaType> for TrackMediaType {
    fn from(from: &RtcOutboundRtpStreamMediaType) -> Self {
        match from {
            RtcOutboundRtpStreamMediaType::Audio { .. } => Self::Audio,
            RtcOutboundRtpStreamMediaType::Video { .. } => Self::Video,
        }
    }
}

impl From<&RtcInboundRtpStreamMediaType> for TrackMediaType {
    fn from(from: &RtcInboundRtpStreamMediaType) -> Self {
        match from {
            RtcInboundRtpStreamMediaType::Audio { .. } => Self::Audio,
            RtcInboundRtpStreamMediaType::Video { .. } => Self::Video,
        }
    }
}

impl From<&medea_client_api_proto::MediaType> for TrackMediaType {
    fn from(from: &medea_client_api_proto::MediaType) -> Self {
        match from {
            medea_client_api_proto::MediaType::Audio(_) => Self::Audio,
            medea_client_api_proto::MediaType::Video(_) => Self::Video,
        }
    }
}

// TODO: unit tests
