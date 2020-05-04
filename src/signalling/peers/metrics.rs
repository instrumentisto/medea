//! Service which is responsible for processing [`Peer`]'s [`RtcStat`] metrics.
//!
//! This service acts as flow and stop metrics source for the
//! [`PeerTrafficWatcher`].

// TODO: remove in #91
#![allow(dead_code)]

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
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
    MediaType as MediaTypeProto, PeerId,
};
use medea_macro::dispatchable;

use crate::{
    api::control::{
        callback::{MediaDirection, MediaType},
        RoomId,
    },
    log::prelude::*,
    media::PeerStateMachine,
    signalling::peers::FlowMetricSource,
    utils::convert_instant_to_utc,
};

use super::traffic_watcher::PeerTrafficWatcher;

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

/// Events which [`PeersMetricsService`] can throw to the
/// [`PeersMetricsService::peer_metric_events_sender`]'s receiver (currently
/// this is [`Room`] which owns this [`PeersMetricsService`]).
#[dispatchable]
#[derive(Debug, Clone)]
pub enum PeersMetricsEvent {
    /// Wrong [`Peer`] traffic flowing was detected. Some `MediaTrack`s with
    /// provided [`TrackMediaType`] doesn't flows.
    WrongTrafficFlowing {
        peer_id: PeerId,
        at: DateTime<Utc>,
        media_type: MediaType,
        direction: MediaDirection,
    },

    /// Stopped `MediaTrack` with provided [`MediaType`] and [`MediaDirection`]
    /// was started after stopping.
    TrackTrafficStarted {
        peer_id: PeerId,
        media_type: MediaType,
        direction: MediaDirection,
    },
}

/// Specification of a [`Peer`].
///
/// Based on this specification wrong [`Peer`]'s media traffic flowing
/// will be determined.
#[derive(Debug)]
struct PeerTracks {
    audio_send: u64,
    video_send: u64,
    audio_recv: u64,
    video_recv: u64,
}

impl From<&PeerStateMachine> for PeerTracks {
    fn from(peer: &PeerStateMachine) -> Self {
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
    spec: PeerTracks,

    /// All [`TrackStat`]s with [`Send`] direction of this [`PeerStat`].
    senders: HashMap<StatId, TrackStat<Send>>,

    /// All [`TrackStat`]s with [`Recv`] of this [`PeerStat`].
    receivers: HashMap<StatId, TrackStat<Recv>>,

    /// Current connection state of this [`PeerStat`].
    state: PeerStatState,

    /// Time of the last metrics update of this [`PeerStat`].
    last_update: DateTime<Utc>,

    /// Duration after which media server will consider [`Peer`]'s media
    /// traffic stats as invalid and will send notification about this by
    /// `on_stop` Control API callback.
    peer_validity_timeout: Duration,
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
            .entry(stat_id)
            .or_insert_with(|| TrackStat {
                updated_at: Instant::now(),
                direction: Send { packets_sent: 0 },
                media_type: TrackMediaType::from(&upd.media_type),
            })
            .update(upd);
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
            .entry(stat_id)
            .or_insert_with(|| TrackStat {
                updated_at: Instant::now(),
                direction: Recv {
                    packets_received: 0,
                },
                media_type: TrackMediaType::from(&upd.media_specific_stats),
            })
            .update(upd);
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
    /// [`PeerStat::peer_validity_timeout`] or this [`TrackStat`] will be
    /// considered as stopped.
    fn is_track_active<T>(&self, track: &TrackStat<T>) -> bool {
        track.updated_at().elapsed() < self.peer_validity_timeout
    }

    /// Returns [`MediaDirection`]s and [`MediaType`]s of the `MediaTrack`s
    /// which are currently is stopped.
    ///
    /// This is determined by comparing count of senders/receivers from the
    /// [`PeerSpec`].
    ///
    /// Also media type of sender/receiver
    /// and activity taken into account.
    fn get_stopped_tracks_types(&self) -> HashMap<MediaDirection, MediaType> {
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

        let mut stopped = HashMap::new();
        if audio_send < self.spec.audio_send {
            stopped.insert(MediaDirection::Publish, MediaType::Audio);
        }
        if video_send < self.spec.video_send {
            if let Some(media_type) = stopped.get_mut(&MediaDirection::Publish)
            {
                *media_type = MediaType::Both;
            } else {
                stopped.insert(MediaDirection::Publish, MediaType::Video);
            }
        }
        if audio_recv < self.spec.audio_recv {
            stopped.insert(MediaDirection::Play, MediaType::Audio);
        }
        if video_recv < self.spec.video_recv {
            if let Some(media_type) = stopped.get_mut(&MediaDirection::Play) {
                *media_type = MediaType::Both;
            } else {
                stopped.insert(MediaDirection::Play, MediaType::Video);
            }
        }

        stopped
    }

    /// Returns `true` if all senders and receivers is not sending or receiving
    /// anything.
    fn is_stopped(&self) -> bool {
        let active_senders_count = self
            .senders
            .values()
            .filter(|sender| self.is_track_active(&sender))
            .count();
        let active_receivers_count = self
            .receivers
            .values()
            .filter(|recv| self.is_track_active(&recv))
            .count();

        active_receivers_count + active_senders_count == 0
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

/// Service which responsible for processing [`PeerConnection`]'s metrics
/// received from a client.
#[derive(Debug)]
pub struct PeersMetricsService {
    /// [`RoomId`] of [`Room`] to which this [`PeersMetricsService`] belongs
    /// to.
    room_id: RoomId,

    /// [`Addr`] of [`PeersTrafficWatcher`] to which traffic updates will be
    /// sent.
    peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,

    /// All `PeerConnection` for this [`PeersMetricsService`] will process
    /// metrics.
    peers: HashMap<PeerId, Rc<RefCell<PeerStat>>>,

    /// Sender of [`PeerMetricsEvent`]s.
    ///
    /// Currently [`PeerMetricsEvent`] will receive [`Room`] to which this
    /// [`PeersMetricsService`] belongs to.
    peer_metric_events_sender: Option<mpsc::UnboundedSender<PeersMetricsEvent>>,
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
            peer_metric_events_sender: None,
        }
    }

    /// Some [`Peer`]'s traffic doesn't flows in some `MediaTrack`s which are
    /// should work.
    ///
    /// [`PeerMetricsEvent::WrongTrafficFlowing`] will be sent to the
    /// subscriber.
    fn wrong_traffic_flowing(
        &self,
        peer_id: PeerId,
        at: DateTime<Utc>,
        media_type: MediaType,
        direction: MediaDirection,
    ) {
        if let Some(sender) = &self.peer_metric_events_sender {
            let _ =
                sender.unbounded_send(PeersMetricsEvent::WrongTrafficFlowing {
                    peer_id,
                    at,
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
        if let Some(sender) = &self.peer_metric_events_sender {
            let _ =
                sender.unbounded_send(PeersMetricsEvent::TrackTrafficStarted {
                    peer_id,
                    media_type,
                    direction,
                });
        }
    }

    /// Returns [`Stream`] of [`PeerMetricsEvent`]s.
    ///
    /// Currently this method will be called by a [`Room`] to which this
    /// [`PeersMetricsService`] belongs to.
    pub fn subscribe(&mut self) -> impl Stream<Item = PeersMetricsEvent> {
        let (tx, rx) = mpsc::unbounded();
        self.peer_metric_events_sender = Some(tx);

        rx
    }

    /// Checks that all [`PeerStat`]s is valid accordingly `PeerConnection`
    /// specification. If [`PeerStat`] is considered as invalid accordingly to
    /// `PeerConnection` specification then
    /// [`PeersMetricsService::wrong_traffic_flowing`] will be called.
    ///
    /// Also checks that all [`PeerStat`]'s senders/receivers is flowing. If all
    /// senders/receivers is stopped then [`TrafficStopped`] will be sent to
    /// the [`PeersTrafficWatcher`].
    pub fn check_peers_validity(&mut self) {
        let mut stopped_peers = Vec::new();
        for peer in self
            .peers
            .values()
            .filter(|peer| peer.borrow().state == PeerStatState::Connected)
        {
            let peer_ref = peer.borrow();

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
                let stopped_tracks_types = peer_ref.get_stopped_tracks_types();
                for (direction, media_type) in stopped_tracks_types {
                    debug!(
                        "Peer [id = {}] from Room [id = {}] traffic [{:?}, \
                         {:?}] stopped because wrong traffic flowing.",
                        peer_ref.peer_id, self.room_id, direction, media_type,
                    );
                    let stopped_at =
                        peer_ref.get_tracks_last_update(direction, media_type);
                    self.wrong_traffic_flowing(
                        peer_ref.peer_id,
                        convert_instant_to_utc(stopped_at),
                        media_type,
                        direction,
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
    pub fn register_peer(
        &mut self,
        peer: &PeerStateMachine,
        peer_validity_timeout: Duration,
    ) {
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
            let stopped_tracks_before = peer_ref.get_stopped_tracks_types();

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
                let stopped_tracks_kinds = peer_ref.get_stopped_tracks_types();
                if stopped_tracks_kinds.is_empty() {
                    self.peers_traffic_watcher.traffic_flows(
                        self.room_id.clone(),
                        peer_id,
                        FlowMetricSource::Peer,
                    );
                    if let Some(partner_peer_id) =
                        peer_ref.get_partner_peer_id()
                    {
                        self.peers_traffic_watcher.traffic_flows(
                            self.room_id.clone(),
                            partner_peer_id,
                            FlowMetricSource::PartnerPeer,
                        );
                    }
                } else {
                    for (direction, media_type) in &stopped_tracks_kinds {
                        let stopped_at = peer_ref
                            .get_tracks_last_update(*direction, *media_type);
                        self.wrong_traffic_flowing(
                            peer_ref.peer_id,
                            convert_instant_to_utc(stopped_at),
                            *media_type,
                            *direction,
                        );
                    }

                    for (direction, media_type_before) in stopped_tracks_before
                    {
                        if let Some(started_media_type) = stopped_tracks_kinds
                            .get(&direction)
                            .and_then(|k| media_type_before.get_started(*k))
                        {
                            self.track_traffic_starts_flowing(
                                peer_id,
                                started_media_type,
                                direction,
                            );
                        }
                    }
                }
            }
        }
    }

    /// [`Room`] notifies [`PeersMetricsService`] that some [`Peer`] is removed.
    pub fn unregister_peers(&mut self, peers_ids: HashSet<PeerId>) {
        debug!(
            "Peers [ids = [{:?}]] from Room [id = {}] was unsubscribed from \
             the PeerMetricsService.",
            peers_ids, self.room_id
        );

        for peer_id in &peers_ids {
            self.peers.remove(peer_id);
        }
        self.peers_traffic_watcher
            .unregister_peers(self.room_id.clone(), peers_ids);
    }

    pub fn update_peer_spec(&mut self, peer: &PeerStateMachine) {
        if let Some(peer_stat) = self.peers.get(&peer.id()) {
            peer_stat.borrow_mut().spec = PeerTracks::from(peer);
        }
    }

    pub fn is_peer_registered(&self, peer_id: PeerId) -> bool {
        self.peers.contains_key(&peer_id)
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
