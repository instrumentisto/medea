//! Service which is responsible for processing [`Peer`]s [`RtcStat`] metrics.
//!
//! At first you must register Peer via [`PeersMetricsService.register_peer()`].
//! Use [`PeersMetricsService.subscribe()`] to subscribe to stats processing
//! results. Then provide Peer metrics to [`PeersMetricsService.add_stats()`].
//! You should call [`PeersMetricsService.check_peers()`] with
//! reasonable interval (~1-2 sec), this will check for stale metrics.
//!
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
    MediaType as MediaTypeProto, PeerId,
};
use medea_macro::dispatchable;

use crate::{
    api::control::{
        callback::{MediaDirection, MediaType},
        RoomId,
    },
    log::prelude::*,
    media::PeerStateMachine as Peer,
    signalling::peers::{
        media_traffic_state::{
            get_diff_added, get_diff_removed, MediaTrafficState,
        },
        FlowMetricSource,
    },
    utils::instant_into_utc,
};

use super::traffic_watcher::PeerTrafficWatcher;

/// Service which is responsible for processing [`Peer`]s [`RtcStat`] metrics.
#[derive(Debug)]
pub struct PeersMetricsService {
    /// [`RoomId`] of Room to which this [`PeersMetricsService`] belongs
    /// to.
    room_id: RoomId,

    /// [`PeerTrafficWatcher`] to which traffic updates will be
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
        for peer in self
            .peers
            .values()
            .filter(|peer| peer.borrow().state == PeerStatState::Connected)
        {
            let mut peer_ref = peer.borrow_mut();

            // get state before applying new stats so we can make before-after
            // diff
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
            } else {
                let send_media_traffic_state_after =
                    peer_ref.send_traffic_state;
                let recv_media_traffic_state_after =
                    peer_ref.recv_traffic_state;
                if let Some(stopped_send_media_type) = get_diff_removed(
                    send_media_traffic_state_before,
                    send_media_traffic_state_after,
                ) {
                    self.send_no_traffic(
                        &*peer_ref,
                        stopped_send_media_type,
                        MediaDirection::Publish,
                    );
                }
                if let Some(stopped_recv_media_type) = get_diff_removed(
                    recv_media_traffic_state_before,
                    recv_media_traffic_state_after,
                ) {
                    self.send_no_traffic(
                        &*peer_ref,
                        stopped_recv_media_type,
                        MediaDirection::Play,
                    );
                }
            }
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
            tracks_spec: PeerTracks::from(peer),
            stats_ttl,
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
    ///
    /// Notifies [`PeerTrafficWatcher`] about traffic flowing/stopping.
    ///
    /// Also, from this function can be sent
    /// [`PeersMetricsEvent::WrongTrafficFlowing`] or [`PeersMetricsEvent::
    /// TrackTrafficStarted`] to the [`Room`] if some
    /// [`MediaType`]/[`Direction`] was stopped.
    pub fn add_stats(&mut self, peer_id: PeerId, stats: Vec<RtcStat>) {
        if let Some(peer) = self.peers.get(&peer_id) {
            let mut peer_ref = peer.borrow_mut();

            // get state before applying new stats so we can make before-after
            // diff
            let send_before = peer_ref.send_traffic_state;
            let recv_before = peer_ref.recv_traffic_state;
            peer_ref.update_media_traffic_state();

            // apply new stats
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
            peer_ref.update_recv_traffic_state();
            peer_ref.update_send_traffic_state();

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
                peer_ref.state = PeerStatState::Connected;
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

                let send_after = peer_ref.send_traffic_state;
                let recv_after = peer_ref.recv_traffic_state;

                // compare before and after
                if let Some(started_media_type) =
                    get_diff_added(send_before, send_after)
                {
                    self.send_traffic_flows(
                        peer_id,
                        started_media_type,
                        MediaDirection::Publish,
                    );
                }

                if let Some(started_media_type) =
                    get_diff_added(recv_before, recv_after)
                {
                    self.send_traffic_flows(
                        peer_id,
                        started_media_type,
                        MediaDirection::Play,
                    );
                }

                if let Some(stopped_media_type) =
                    get_diff_removed(send_before, send_after)
                {
                    self.send_no_traffic(
                        &*peer_ref,
                        stopped_media_type,
                        MediaDirection::Publish,
                    );
                }

                if let Some(stopped_media_type) =
                    get_diff_removed(recv_before, recv_after)
                {
                    self.send_no_traffic(
                        &*peer_ref,
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
    pub fn update_peer_tracks(&mut self, peer: &Peer) {
        if let Some(peer_stat) = self.peers.get(&peer.id()) {
            peer_stat.borrow_mut().tracks_spec = PeerTracks::from(peer);
        }
    }

    /// Sends [`PeerMetricsEvent::NoTrafficFlow`] event to subscriber.
    fn send_no_traffic(
        &self,
        peer: &PeerStat,
        media_type: MediaType,
        direction: MediaDirection,
    ) {
        if let Some(sender) = &self.events_tx {
            let was_flowing_at =
                peer.get_tracks_last_update(direction, media_type);
            let _ = sender.unbounded_send(PeersMetricsEvent::NoTrafficFlow {
                peer_id: peer.peer_id,
                was_flowing_at: instant_into_utc(was_flowing_at),
                media_type,
                direction,
            });
        }
    }

    /// Sends [`PeerMetricsEvent::TrafficFlows`] event to subscriber.
    fn send_traffic_flows(
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
            MediaType::Video => *self == TrackMediaType::Video,
            MediaType::Audio => *self == TrackMediaType::Audio,
            MediaType::Both => true,
        }
    }
}

/// Events which [`PeersMetricsService`] can send to its subscriber.
#[dispatchable]
#[derive(Debug, Clone)]
pub enum PeersMetricsEvent {
    /// Some `MediaTrack`s with provided [`TrackMediaType`] doesn't flows.
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
    /// Count of the [`MediaTrack`]s with the [`Direction::Publish`] and
    /// [`MediaType::Audio`].
    audio_send: usize,

    /// Count of the [`MediaTrack`]s with the [`Direction::Publish`] and
    /// [`MediaType::Video`].
    video_send: usize,

    /// Count of the [`MediaTrack`]s with the [`Direction::Play`] and
    /// [`MediaType::Audio`].
    audio_recv: usize,

    /// Count of the [`MediaTrack`]s with the [`Direction::Play`] and
    /// [`MediaType::Video`].
    video_recv: usize,
}

impl PeerTracks {
    /// Returns count of [`MediaTrack`]s by provided [`TrackMediaType`] and
    /// [`MediaDirection`].
    fn get_by_kind(
        &self,
        kind: TrackMediaType,
        direction: MediaDirection,
    ) -> usize {
        match (direction, kind) {
            (MediaDirection::Publish, TrackMediaType::Audio) => self.audio_send,
            (MediaDirection::Publish, TrackMediaType::Video) => self.video_send,
            (MediaDirection::Play, TrackMediaType::Audio) => self.audio_recv,
            (MediaDirection::Play, TrackMediaType::Video) => self.video_recv,
        }
    }
}

impl From<&Peer> for PeerTracks {
    fn from(peer: &Peer) -> Self {
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

    /// Duration, after which this stat will be considered as stale.
    ttl: Duration,

    /// Media type of the `MediaTrack` which this [`TrackStat`] represents.
    media_type: TrackMediaType,

    /// Direction state of this [`TrackStat`].
    ///
    /// Can be [`Send`] or [`Recv`].
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

impl<T> TrackStat<T> {
    /// Checks that media traffic flows through provided [`TrackStat`].
    ///
    /// [`TrackStat`] should be updated within
    /// [`PeerStat::stats_ttl`] or this [`TrackStat`] will be
    /// considered as stopped.
    // TODO: more asserts will be added when all browsers will adopt new stats
    //       spec https://www.w3.org/TR/webrtc-stats/
    fn is_flowing(&self) -> bool {
        self.updated_at().elapsed() < self.ttl
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

    /// State of the traffic flowing with the [`Send`] direction for
    /// [`MediaType`]s.
    send_traffic_state: MediaTrafficState,

    /// State of the traffic flowing with the [`Recv`] direction for
    /// [`MediaType`]s.
    recv_traffic_state: MediaTrafficState,

    /// All [`TrackStat`]s with [`Recv`] of this [`PeerStat`].
    receivers: HashMap<StatId, TrackStat<Recv>>,

    /// Current connection state of this [`PeerStat`].
    state: PeerStatState,

    /// Time of the last metrics update of this [`PeerStat`].
    last_update: DateTime<Utc>,

    /// Duration, after which [`Peer`]s stats will be considered as stale.
    stats_ttl: Duration,
}

impl PeerStat {
    /// Updates [`TrackStat`] with provided [`StatId`] by
    /// [`RtcOutboundRtpStreamStats`].
    ///
    /// Updates [`MediaTrafficState`] of the [`Send`] direction.
    fn update_sender(
        &mut self,
        stat_id: StatId,
        upd: &RtcOutboundRtpStreamStats,
    ) {
        self.last_update = Utc::now();
        let ttl = self.stats_ttl;
        let sender = self.senders.entry(stat_id).or_insert_with(|| TrackStat {
            updated_at: Instant::now(),
            ttl,
            direction: Send { packets_sent: 0 },
            media_type: TrackMediaType::from(&upd.media_type),
        });
        sender.update(upd);
    }

    /// Updates [`TrackStat`] with provided [`StatId`] by
    /// [`RtcInboundRtpStreamStats`].
    ///
    /// Updates [`MediaTrafficState`] of the [`Recv`] direction.
    fn update_receiver(
        &mut self,
        stat_id: StatId,
        upd: &RtcInboundRtpStreamStats,
    ) {
        self.last_update = Utc::now();
        let ttl = self.stats_ttl;
        let receiver =
            self.receivers.entry(stat_id).or_insert_with(|| TrackStat {
                updated_at: Instant::now(),
                ttl,
                direction: Recv {
                    packets_received: 0,
                },
                media_type: TrackMediaType::from(&upd.media_specific_stats),
            });
        receiver.update(upd);
    }

    /// Updates `recv_traffic_state` based on current `receivers` state.
    /// Supposed to be called after you finished updating `receivers`.
    fn update_recv_traffic_state(&mut self) {
        for track_media_type in &[TrackMediaType::Video, TrackMediaType::Audio]
        {
            let media_type = (*track_media_type).into();
            let cnt_flowing = self
                .receivers
                .values()
                .filter(|rx| rx.media_type == *track_media_type)
                .filter(|rx| rx.is_flowing())
                .count();
            if cnt_flowing != 0
                && cnt_flowing
                    >= self
                        .tracks_spec
                        .get_by_kind(*track_media_type, MediaDirection::Play)
            {
                self.recv_traffic_state.started(media_type);
            } else {
                self.recv_traffic_state.stopped(media_type);
            }
        }
    }

    /// Updates `send_traffic_state` based on current `senders` state. Supposed
    /// to be called after you finished updating `senders`.
    fn update_send_traffic_state(&mut self) {
        for track_media_type in &[TrackMediaType::Video, TrackMediaType::Audio]
        {
            let media_type = (*track_media_type).into();
            let cnt_flowing = self
                .senders
                .values()
                .filter(|rx| rx.media_type == *track_media_type)
                .filter(|rx| rx.is_flowing())
                .count();
            if cnt_flowing != 0
                && cnt_flowing
                    >= self
                        .tracks_spec
                        .get_by_kind(*track_media_type, MediaDirection::Publish)
            {
                self.send_traffic_state.started(media_type);
            } else {
                self.send_traffic_state.stopped(media_type);
            }
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
            .filter(|sender| sender.is_flowing())
            .for_each(|sender| match sender.media_type {
                TrackMediaType::Audio => audio_send += 1,
                TrackMediaType::Video => video_send += 1,
            });
        self.receivers
            .values()
            .filter(|receiver| receiver.is_flowing())
            .for_each(|receiver| match receiver.media_type {
                TrackMediaType::Audio => audio_recv += 1,
                TrackMediaType::Video => video_recv += 1,
            });

        if audio_send < self.tracks_spec.audio_send {
            self.send_traffic_state.stopped(MediaType::Audio);
        } else {
            self.send_traffic_state.started(MediaType::Audio);
        }
        if video_send < self.tracks_spec.video_send {
            self.send_traffic_state.stopped(MediaType::Video);
        } else {
            self.send_traffic_state.started(MediaType::Video);
        }
        if audio_recv < self.tracks_spec.audio_recv {
            self.recv_traffic_state.stopped(MediaType::Audio);
        } else {
            self.recv_traffic_state.started(MediaType::Audio);
        }
        if video_recv < self.tracks_spec.video_recv {
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

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        sync::Arc,
        time::{Duration, SystemTime},
    };

    use chrono::{DateTime, Utc};
    use futures::{channel::mpsc, stream::LocalBoxStream, StreamExt as _};
    use medea_client_api_proto::{
        stats::{
            RtcInboundRtpStreamMediaType, RtcInboundRtpStreamStats,
            RtcOutboundRtpStreamMediaType, RtcOutboundRtpStreamStats, RtcStat,
            RtcStatsType, StatId,
        },
        PeerId,
    };
    use tokio::time::{delay_for, timeout};

    use crate::{
        api::control::callback::{MediaDirection, MediaType},
        media::peer::tests::test_peer_from_peer_tracks,
        signalling::peers::{
            traffic_watcher::MockPeerTrafficWatcher, PeersMetricsEvent,
        },
    };

    use super::PeersMetricsService;

    /// Returns [`RtcOutboundRtpStreamStats`] with a provided number of
    /// `packets_sent` and [`RtcOutboundRtpStreamMediaType`] based on
    /// `is_audio`.
    fn build_outbound_stream_stat(
        packets_sent: u64,
        is_audio: bool,
    ) -> RtcOutboundRtpStreamStats {
        let media_type = if is_audio {
            RtcOutboundRtpStreamMediaType::Audio {
                total_samples_sent: None,
                voice_activity_flag: None,
            }
        } else {
            RtcOutboundRtpStreamMediaType::Video {
                frame_width: None,
                frame_height: None,
                frames_per_second: None,
            }
        };

        RtcOutboundRtpStreamStats {
            track_id: None,
            media_type,
            packets_sent,
            bytes_sent: 0,
            media_source_id: None,
        }
    }

    /// Returns [`RtcInboundRtpStreamStats`] with a provided number of
    /// `packets_received` and [`RtcInboundRtpStreamMediaType`] based on
    /// `is_audio`.
    fn build_inbound_stream_stat(
        packets_received: u64,
        is_audio: bool,
    ) -> RtcInboundRtpStreamStats {
        let media_type = if is_audio {
            RtcInboundRtpStreamMediaType::Audio {
                voice_activity_flag: None,
                total_samples_received: None,
                concealed_samples: None,
                silent_concealed_samples: None,
                audio_level: None,
                total_audio_energy: None,
                total_samples_duration: None,
            }
        } else {
            RtcInboundRtpStreamMediaType::Video {
                frames_decoded: None,
                key_frames_decoded: None,
                frame_width: None,
                frame_height: None,
                total_inter_frame_delay: None,
                frames_per_second: None,
                frame_bit_depth: None,
                fir_count: None,
                pli_count: None,
                sli_count: None,
                concealment_events: None,
                frames_received: None,
            }
        };

        RtcInboundRtpStreamStats {
            packets_received,
            track_id: None,
            media_specific_stats: media_type,
            bytes_received: 0,
            packets_lost: None,
            jitter: None,
            total_decode_time: None,
            jitter_buffer_emitted_count: None,
        }
    }

    /// Helper for the all [`metrics`] unit tests.
    struct Helper {
        /// Stream to which will be sent `()` on every [`TrafficFlows`] message
        /// received from the [`PeersMetricsService`]
        traffic_flows_stream: LocalBoxStream<'static, ()>,

        /// Stream to which will be sent `()` on every [`TrafficStopped`]
        /// message received from the [`PeersMetricsService`]
        traffic_stopped_stream: LocalBoxStream<'static, ()>,

        /// Stream to which will [`PeerMetricsService`] will send all his
        /// [`PeerMetricsEvent`]s.
        peer_events_stream: LocalBoxStream<'static, PeersMetricsEvent>,

        /// Actual [`PeerMetricsService`].
        metrics: PeersMetricsService,
    }

    impl Helper {
        /// Returns new [`Helper`] with [`PeerMetricsService`] in which [`Room`]
        /// with `test` ID was registered.
        pub fn new() -> Self {
            let mut watcher = MockPeerTrafficWatcher::new();
            watcher
                .expect_register_room()
                .returning(|_, _| Box::pin(async { Ok(()) }));
            watcher.expect_unregister_room().return_const(());
            watcher
                .expect_register_peer()
                .returning(|_, _, _| Box::pin(async { Ok(()) }));
            watcher.expect_unregister_peers().return_const(());
            let (traffic_flows_tx, traffic_flows_rx) = mpsc::unbounded();
            watcher.expect_traffic_flows().returning(move |_, _, _| {
                traffic_flows_tx.unbounded_send(()).unwrap();
            });
            let (traffic_stopped_tx, traffic_stopped_rx) = mpsc::unbounded();
            watcher.expect_traffic_stopped().returning(move |_, _, _| {
                traffic_stopped_tx.unbounded_send(()).unwrap();
            });

            let mut metrics = PeersMetricsService::new(
                "test".to_string().into(),
                Arc::new(watcher),
            );

            Self {
                traffic_flows_stream: Box::pin(traffic_flows_rx),
                traffic_stopped_stream: Box::pin(traffic_stopped_rx),
                peer_events_stream: Box::pin(metrics.subscribe()),
                metrics,
            }
        }

        /// Registers [`Peer`] with `PeerId(1)` and provided [`MediaTrack`]s
        /// count.
        pub fn register_peer(
            &mut self,
            stats_ttl: Duration,
            send_audio: u32,
            send_video: u32,
            recv_audio: u32,
            recv_video: u32,
        ) {
            self.metrics.register_peer(
                &test_peer_from_peer_tracks(
                    send_audio, send_video, recv_audio, recv_video,
                ),
                stats_ttl,
            );
        }

        /// Generates [`RtcStats`] and adds them to inner
        /// [`PeersMetricsService`] for `PeerId(1)`.
        pub fn add_stats(
            &mut self,
            send_audio: u32,
            send_video: u32,
            recv_audio: u32,
            recv_video: u32,
            packets: u64,
        ) {
            let mut stats = Vec::new();
            for i in 0..send_audio {
                stats.push(RtcStat {
                    id: StatId(format!("{}-send-audio", i)),
                    timestamp: SystemTime::now().into(),
                    stats: RtcStatsType::OutboundRtp(Box::new(
                        build_outbound_stream_stat(packets, true),
                    )),
                });
            }

            for i in 0..send_video {
                stats.push(RtcStat {
                    id: StatId(format!("{}-send-video", i)),
                    timestamp: SystemTime::now().into(),
                    stats: RtcStatsType::OutboundRtp(Box::new(
                        build_outbound_stream_stat(packets, false),
                    )),
                })
            }

            for i in 0..recv_audio {
                stats.push(RtcStat {
                    id: StatId(format!("{}-recv-audio", i)),
                    timestamp: SystemTime::now().into(),
                    stats: RtcStatsType::InboundRtp(Box::new(
                        build_inbound_stream_stat(packets, true),
                    )),
                })
            }

            for i in 0..recv_video {
                stats.push(RtcStat {
                    id: StatId(format!("{}-recv-video", i)),
                    timestamp: SystemTime::now().into(),
                    stats: RtcStatsType::InboundRtp(Box::new(
                        build_inbound_stream_stat(packets, false),
                    )),
                })
            }

            self.metrics.add_stats(PeerId(1), stats);
        }

        /// Waits for `traffic_flows()` invocation on inner
        /// [`PeerTrafficWatcher`].
        pub async fn traffic_flows_invoked(&mut self) {
            self.traffic_flows_stream.next().await;
        }

        pub async fn next_no_traffic_event(
            &mut self,
        ) -> (PeerId, DateTime<Utc>, MediaType, MediaDirection) {
            let event = self.peer_events_stream.next().await.unwrap();
            if let PeersMetricsEvent::NoTrafficFlow {
                peer_id,
                was_flowing_at,
                media_type,
                direction,
            } = event
            {
                (peer_id, was_flowing_at, media_type, direction)
            } else {
                unreachable!("Unexpected event received: {:?}.", event)
            }
        }

        pub async fn next_traffic_event(
            &mut self,
        ) -> (PeerId, MediaType, MediaDirection) {
            let event = self.peer_events_stream.next().await.unwrap();
            if let PeersMetricsEvent::TrafficFlows {
                peer_id,
                media_type,
                direction,
            } = event
            {
                (peer_id, media_type, direction)
            } else {
                unreachable!("Unexpected event received: {:?}.", event)
            }
        }

        /// Waits for the `traffic_stopped()` invoked on inner
        /// [`PeerTrafficWatcher`].
        pub async fn traffic_stoped_invoked(&mut self) {
            self.traffic_stopped_stream.next().await;
        }

        /// Returns next [`PeerMetricsEvent`] which [`PeerMetricsService`] wants
        /// send to the [`Room`].
        pub async fn next_event(&mut self) -> PeersMetricsEvent {
            self.peer_events_stream.next().await.unwrap()
        }

        /// Calls [`PeerMetricsService::check_peers`].
        pub fn check_peers(&mut self) {
            self.metrics.check_peers();
        }

        /// Calls [`PeerMetricsService::unregister_peers`] with provided
        /// [`PeerId`] as argument.
        pub fn unregister_peer(&mut self, peer_id: PeerId) {
            self.metrics.unregister_peers(&[peer_id]);
        }
    }

    #[allow(clippy::struct_excessive_bools)]
    #[derive(Debug, Default, PartialEq)]
    struct MergedFlowState {
        audio_send: bool,
        video_send: bool,
        audio_recv: bool,
        video_recv: bool,
    }

    impl MergedFlowState {
        fn add_event(
            &mut self,
            event: Option<(PeerId, MediaType, MediaDirection)>,
        ) {
            if let Some((_, media, direction)) = event {
                match (media, direction) {
                    (MediaType::Audio, MediaDirection::Play) => {
                        self.audio_recv = true;
                    }
                    (MediaType::Video, MediaDirection::Play) => {
                        self.video_recv = true;
                    }
                    (MediaType::Both, MediaDirection::Play) => {
                        self.video_recv = true;
                        self.audio_recv = true;
                    }
                    (MediaType::Audio, MediaDirection::Publish) => {
                        self.audio_send = true;
                    }
                    (MediaType::Video, MediaDirection::Publish) => {
                        self.video_send = true;
                    }
                    (MediaType::Both, MediaDirection::Publish) => {
                        self.audio_send = true;
                        self.video_send = true;
                    }
                }
            }
        }
    }

    async fn traffic_flows_helper(
        stats_tll: Option<Duration>,
        spec: (u32, u32, u32, u32),
        stats: (u32, u32, u32, u32),
        should_flow: bool,
    ) -> (MergedFlowState, Helper) {
        let mut helper = Helper::new();
        helper.register_peer(
            stats_tll.unwrap_or(Duration::from_secs(999)),
            spec.0,
            spec.1,
            spec.2,
            spec.3,
        );
        helper.add_stats(stats.0, stats.1, stats.2, stats.3, 100);

        let traffic_flow =
            timeout(Duration::from_millis(10), helper.traffic_flows_invoked())
                .await;
        if should_flow {
            traffic_flow.unwrap();
        } else {
            traffic_flow.unwrap_err();
        }

        let flow1 =
            timeout(Duration::from_millis(10), helper.next_traffic_event())
                .await
                .ok();
        let flow2 =
            timeout(Duration::from_millis(10), helper.next_traffic_event())
                .await
                .ok();

        let mut result = MergedFlowState::default();

        result.add_event(flow1);
        result.add_event(flow2);
        (result, helper)
    }

    /// Checks that [`PeerMetricsEvent::TrafficFlows`] are emitted when calling
    /// `add_stats` with required stats.
    #[actix_rt::test]
    async fn traffic_flows() {
        assert_eq!(
            traffic_flows_helper(None, (1, 1, 1, 1), (1, 1, 1, 1), true)
                .await
                .0,
            MergedFlowState {
                audio_send: true,
                video_send: true,
                audio_recv: true,
                video_recv: true
            }
        );
        assert_eq!(
            traffic_flows_helper(None, (1, 1, 1, 1), (0, 0, 0, 0), false)
                .await
                .0,
            MergedFlowState {
                audio_send: false,
                video_send: false,
                audio_recv: false,
                video_recv: false
            }
        );
        assert_eq!(
            traffic_flows_helper(None, (2, 1, 2, 1), (1, 0, 1, 2), true)
                .await
                .0,
            MergedFlowState {
                audio_send: false,
                video_send: false,
                audio_recv: false,
                video_recv: true
            }
        );
    }

    /// Checks that [`PeerMetricsEvent::NoTrafficFlow`] are sent on partial
    /// traffic flowing stops.
    #[actix_rt::test]
    async fn traffic_stops() {
        async fn partial_stop_helper(
            spec: (u32, u32, u32, u32),
            stats: (u32, u32, u32, u32),
            should_stop_flowing: bool,
        ) -> MergedFlowState {
            let (flow_state, mut helper) = traffic_flows_helper(
                Some(Duration::from_millis(10)),
                spec,
                spec,
                true,
            )
            .await;
            assert_eq!(
                flow_state,
                MergedFlowState {
                    audio_send: true,
                    video_send: true,
                    audio_recv: true,
                    video_recv: true,
                }
            );
            delay_for(Duration::from_millis(15)).await;
            helper.add_stats(stats.0, stats.1, stats.2, stats.3, 200);

            let traffic_flow = timeout(
                Duration::from_millis(10),
                helper.traffic_stoped_invoked(),
            )
            .await;
            if should_stop_flowing {
                traffic_flow.unwrap();
            } else {
                traffic_flow.unwrap_err();
            }

            let flow1 = timeout(
                Duration::from_millis(10),
                helper.next_no_traffic_event(),
            )
            .await
            .ok()
            .map(|ev| (ev.0, ev.2, ev.3));
            let flow2 = timeout(
                Duration::from_millis(10),
                helper.next_no_traffic_event(),
            )
            .await
            .ok()
            .map(|ev| (ev.0, ev.2, ev.3));

            let mut result = MergedFlowState::default();
            result.add_event(flow1);
            result.add_event(flow2);

            result
        }

        assert_eq!(
            partial_stop_helper((1, 1, 1, 1), (0, 0, 0, 0), true).await,
            MergedFlowState {
                audio_send: false,
                video_send: false,
                audio_recv: false,
                video_recv: false
            }
        );
        assert_eq!(
            partial_stop_helper((1, 1, 1, 1), (1, 1, 1, 0), false).await,
            MergedFlowState {
                audio_send: false,
                video_send: false,
                audio_recv: false,
                video_recv: true
            }
        );
        assert_eq!(
            partial_stop_helper((1, 1, 2, 1), (0, 0, 1, 1), false).await,
            MergedFlowState {
                audio_send: true,
                video_send: true,
                audio_recv: true,
                video_recv: false
            }
        );
    }

    /// Checks that [`PeerMetricsService::unregister_peer`] doesn't triggers
    /// anything ([`TrafficStopped`], [`PeerEventsEvent::NoTrafficFlow`] etc.).
    #[actix_rt::test]
    async fn peer_unregister_doesnt_trigger_anything() {
        let mut helper = Helper::new();
        helper.register_peer(Duration::from_millis(50), 1, 1, 1, 1);
        helper.add_stats(1, 1, 1, 1, 100);

        let mut directions = HashSet::new();
        loop {
            let event = helper.next_event().await;
            match event {
                PeersMetricsEvent::TrafficFlows {
                    peer_id,
                    media_type,
                    direction,
                } => {
                    assert_eq!(peer_id, PeerId(1));
                    assert_eq!(media_type, MediaType::Both);
                    directions.insert(direction);
                    if directions.len() == 2 {
                        break;
                    }
                }
                _ => panic!("Unknown event received: {:?}", event),
            }
        }

        helper.unregister_peer(PeerId(1));
        timeout(Duration::from_millis(10), helper.next_event())
            .await
            .unwrap_err();
        timeout(Duration::from_millis(10), helper.traffic_stoped_invoked())
            .await
            .unwrap_err();
    }

    /// Calling `check_peers` after adding new tracks via `update_peer_tracks`
    /// doesn't emits [`PeersMetricsEvent::NoTrafficFlow`].
    #[actix_rt::test]
    async fn update_peer_tracks_and_check_peers() {
        let (state, mut helper) =
            traffic_flows_helper(None, (0, 0, 1, 1), (0, 0, 1, 1), true).await;

        assert_eq!(
            state,
            MergedFlowState {
                audio_send: false,
                video_send: false,
                audio_recv: true,
                video_recv: true
            }
        );

        helper
            .metrics
            .update_peer_tracks(&test_peer_from_peer_tracks(1, 1, 1, 1));
        helper.check_peers();
        timeout(Duration::from_millis(10), helper.next_no_traffic_event())
            .await
            .unwrap_err();
        timeout(Duration::from_millis(10), helper.next_traffic_event())
            .await
            .unwrap_err();
    }

    /// Calling `add_stats` after adding new tracks via `update_peer_tracks`
    /// doesn't emit [`PeersMetricsEvent::NoTrafficFlow`].
    #[actix_rt::test]
    async fn update_peer_tracks_and_add_stats() {
        let (state, mut helper) =
            traffic_flows_helper(None, (0, 0, 1, 1), (0, 0, 1, 1), true).await;

        assert_eq!(
            state,
            MergedFlowState {
                audio_send: false,
                video_send: false,
                audio_recv: true,
                video_recv: true
            }
        );

        helper
            .metrics
            .update_peer_tracks(&test_peer_from_peer_tracks(1, 1, 1, 1));
        helper.add_stats(0, 0, 1, 1, 300);
        timeout(Duration::from_millis(10), helper.next_no_traffic_event())
            .await
            .unwrap_err();
        timeout(Duration::from_millis(10), helper.next_traffic_event())
            .await
            .unwrap_err();
    }
}
