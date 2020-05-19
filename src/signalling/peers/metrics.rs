//! Service which is responsible for processing [`Peer`]s [`RtcStat`] metrics.
//!
//! At first you must register Peer via [`PeersMetricsService.register_peer()`].
//! Use [`PeersMetricsService.subscribe()`] to subscribe to stats processing
//! results. Then provide Peer metrics to [`PeersMetricsService.add_stat()`].
//! You should call [`PeersMetricsService.check_peers()`] with
//! reasonable interval (~1-2 sec), this will check for stale metrics.
//!
//! This service acts as flow and stop metrics source for the
//! [`PeerTrafficWatcher`].

use std::{
    cell::RefCell,
    cmp::Ordering,
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
    media::PeerStateMachine as Peer,
    signalling::peers::{
        media_traffic_state::{
            get_diff_disabled, get_diff_enabled, MediaTrafficState,
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
            } else {
                let send_media_traffic_state_after =
                    peer_ref.send_traffic_state;
                let recv_media_traffic_state_after =
                    peer_ref.recv_traffic_state;
                if let Some(stopped_send_media_type) = get_diff_disabled(
                    send_media_traffic_state_before,
                    send_media_traffic_state_after,
                ) {
                    self.send_no_traffic(
                        &*peer_ref,
                        stopped_send_media_type,
                        MediaDirection::Publish,
                    );
                }
                if let Some(stopped_recv_media_type) = get_diff_disabled(
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
            peer_ref.remove_stopped_stats();
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
    pub fn add_stat(&mut self, peer_id: PeerId, stats: Vec<RtcStat>) {
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
                    get_diff_enabled(send_before, send_after)
                {
                    self.send_traffic_flows(
                        peer_id,
                        started_media_type,
                        MediaDirection::Publish,
                    );
                }

                if let Some(started_media_type) =
                    get_diff_enabled(recv_before, recv_after)
                {
                    self.send_traffic_flows(
                        peer_id,
                        started_media_type,
                        MediaDirection::Play,
                    );
                }

                if let Some(stopped_media_type) =
                    get_diff_disabled(send_before, send_after)
                {
                    self.send_no_traffic(
                        &*peer_ref,
                        stopped_media_type,
                        MediaDirection::Publish,
                    );
                }

                if let Some(stopped_media_type) =
                    get_diff_disabled(recv_before, recv_after)
                {
                    self.send_no_traffic(
                        &*peer_ref,
                        stopped_media_type,
                        MediaDirection::Play,
                    );
                }
            }

            peer_ref.remove_stopped_stats();
        }
    }

    /// Stops tracking provided [`Peer`]s.
    pub fn unregister_peers(&mut self, peers_ids: &HashSet<PeerId>) {
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
    ///
    /// Updates [`MediaTrafficState`] if this [`PeerStat`], sends
    /// [`PeerMetricsEvent::NoTrafficFlow`] removes unneeded [`TrackStat`]s
    /// accordingly to the new [`PeerTracks`] spec.
    pub fn update_peer_tracks(&mut self, peer: &Peer) {
        if let Some(peer_stat) = self.peers.get(&peer.id()) {
            let mut peer_stat_ref = peer_stat.borrow_mut();
            let updated_peer_tracks = PeerTracks::from(peer);
            let send_traffic_state_before = peer_stat_ref.send_traffic_state;
            let recv_traffic_state_before = peer_stat_ref.recv_traffic_state;
            {
                let current_peer_tracks = peer_stat_ref.tracks_spec;

                if updated_peer_tracks.audio_send
                    < current_peer_tracks.audio_send
                {
                    peer_stat_ref.send_traffic_state.disable(MediaType::Audio);
                }
                if updated_peer_tracks.video_send
                    < current_peer_tracks.video_send
                {
                    peer_stat_ref.send_traffic_state.disable(MediaType::Video);
                }

                if updated_peer_tracks.audio_recv
                    < current_peer_tracks.audio_recv
                {
                    peer_stat_ref.recv_traffic_state.disable(MediaType::Audio);
                }
                if updated_peer_tracks.video_recv
                    < current_peer_tracks.video_recv
                {
                    peer_stat_ref.recv_traffic_state.disable(MediaType::Video);
                }
            }

            let send_stopped_media_type = get_diff_disabled(
                send_traffic_state_before,
                peer_stat_ref.send_traffic_state,
            );
            if let Some(send_stopped_media_type) = send_stopped_media_type {
                let senders_to_remove: HashSet<_> = peer_stat_ref
                    .senders
                    .iter()
                    .filter_map(|(id, sender)| {
                        if sender.media_type == send_stopped_media_type {
                            Some(id.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                senders_to_remove.iter().for_each(|id| {
                    peer_stat_ref.remove_sender(id);
                });
                peer_stat_ref
                    .send_traffic_state
                    .disable(send_stopped_media_type);
                self.send_no_traffic(
                    &peer_stat_ref,
                    send_stopped_media_type,
                    MediaDirection::Publish,
                );
            }

            let recv_stopped_media_type = get_diff_disabled(
                recv_traffic_state_before,
                peer_stat_ref.recv_traffic_state,
            );
            if let Some(recv_stopped_media_type) = recv_stopped_media_type {
                let receivers_to_remove: HashSet<_> = peer_stat_ref
                    .receivers
                    .iter()
                    .filter_map(|(id, receiver)| {
                        if receiver.media_type == recv_stopped_media_type {
                            Some(id.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                receivers_to_remove.iter().for_each(|id| {
                    peer_stat_ref.remove_receiver(id);
                });
                peer_stat_ref
                    .recv_traffic_state
                    .disable(recv_stopped_media_type);
                self.send_no_traffic(
                    &peer_stat_ref,
                    recv_stopped_media_type,
                    MediaDirection::Play,
                );
            }

            peer_stat_ref.tracks_spec = updated_peer_tracks;
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
            debug!(
                "Sending NoTraffic event for a Peer [id = {}].",
                peer.peer_id
            );
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
            debug!("Sending TrafficFlows event for a Peer [id = {}].", peer_id);
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
    /// Media traffic of a some `MediaTrack` was partially stopped.
    NoTrafficFlow {
        /// [`PeerId`] of a `Peer` whose media traffic was partially stopped.
        peer_id: PeerId,

        /// [`DateTime`] at which media traffic flowing was partially stopped.
        was_flowing_at: DateTime<Utc>,

        /// [`MediaType`] of a media traffic which stopped flowing.
        media_type: MediaType,

        /// [`MediaDirection`] of a media traffic which stopped flowing.
        direction: MediaDirection,
    },

    /// Stopped `MediaTrack` with provided [`MediaType`] and [`MediaDirection`]
    /// was started after stopping.
    TrafficFlows {
        /// [`PeerId`] of a `Peer` whose media traffic was partially started.
        peer_id: PeerId,

        /// [`MediaType`] of a media traffic which was started flowing.
        media_type: MediaType,

        /// [`MediaDirection`] of a media traffic which was started flowing.
        direction: MediaDirection,
    },
}

/// Specification of a [`Peer`]s tracks. Contains info about how many tracks of
/// each kind should this [`Peer`] send/receive.
///
/// This spec is compared with [`Peer`]s actual stats, to calculate difference
/// between expected and actual [`Peer`] state.
#[derive(Debug, Clone, Copy)]
struct PeerTracks {
    /// Count of the [`MediaTrack`]s with the [`Direction::Publish`] and
    /// [`MediaType::Audio`].
    audio_send: u64,

    /// Count of the [`MediaTrack`]s with the [`Direction::Publish`] and
    /// [`MediaType::Video`].
    video_send: u64,

    /// Count of the [`MediaTrack`]s with the [`Direction::Play`] and
    /// [`MediaType::Audio`].
    audio_recv: u64,

    /// Count of the [`MediaTrack`]s with the [`Direction::Play`] and
    /// [`MediaType::Video`].
    video_recv: u64,
}

impl From<&Peer> for PeerTracks {
    fn from(peer: &Peer) -> Self {
        let mut audio_send = 0;
        let mut video_send = 0;
        let mut audio_recv = 0;
        let mut video_recv = 0;

        for sender in
            peer.senders().values().filter(|sender| !sender.is_muted())
        {
            match sender.media_type {
                MediaTypeProto::Audio(_) => audio_send += 1,
                MediaTypeProto::Video(_) => video_send += 1,
            }
        }
        for receiver in peer
            .receivers()
            .values()
            .filter(|receiver| !receiver.is_muted())
        {
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
    ///
    /// [`TrackStat`] wouldn't be updated if a provided [`MediaType`] and
    /// [`MediaDirection`] of a provided `RTCStat` was stopped accordingly
    /// to the [`PeerTracks`] spec of this [`PeerStat`].
    fn update_sender(
        &mut self,
        stat_id: StatId,
        upd: &RtcOutboundRtpStreamStats,
    ) {
        let track_media_type = TrackMediaType::from(&upd.media_type);
        if self.is_sender_stopped_in_spec(track_media_type) {
            return;
        }

        self.last_update = Utc::now();
        let ttl = self.stats_ttl;
        let sender = self.senders.entry(stat_id).or_insert_with(|| TrackStat {
            updated_at: Instant::now(),
            ttl,
            direction: Send { packets_sent: 0 },
            media_type: track_media_type,
        });
        sender.update(upd);
        if sender.is_flowing() {
            let sender_media_type: MediaType = sender.media_type.into();
            self.send_traffic_state.started(sender_media_type);
        }
    }

    /// Updates [`TrackStat`] with provided [`StatId`] by
    /// [`RtcInboundRtpStreamStats`].
    ///
    /// Updates [`MediaTrafficState`] of the [`Recv`] direction.
    ///
    /// [`TrackStat`] wouldn't be updated if a provided [`MediaType`] and
    /// [`MediaDirection`] of a provided `RTCStat` was stopped accordingly
    /// to the [`PeerTracks`] spec of this [`PeerStat`].
    fn update_receiver(
        &mut self,
        stat_id: StatId,
        upd: &RtcInboundRtpStreamStats,
    ) {
        let track_media_type = TrackMediaType::from(&upd.media_specific_stats);
        if self.is_receiver_stopped_in_spec(track_media_type) {
            return;
        }

        self.last_update = Utc::now();
        let ttl = self.stats_ttl;
        let receiver =
            self.receivers.entry(stat_id).or_insert_with(|| TrackStat {
                updated_at: Instant::now(),
                ttl,
                direction: Recv {
                    packets_received: 0,
                },
                media_type: track_media_type,
            });
        receiver.update(upd);
        if receiver.is_flowing() {
            let receiver_media_type = receiver.media_type.into();
            self.recv_traffic_state.started(receiver_media_type);
        }
    }

    /// Returns `true` if tracks with [`Direction::Send`] and provided
    /// [`TrackMediaType`] is fully stopped.
    fn is_sender_stopped_in_spec(&self, media_type: TrackMediaType) -> bool {
        match media_type {
            TrackMediaType::Audio => self.tracks_spec.audio_send == 0,
            TrackMediaType::Video => self.tracks_spec.video_send == 0,
        }
    }

    /// Returns `true` if tracks with [`Direction::Recv`] and provided
    /// [`TrackMediaType`] is fully stopped.
    fn is_receiver_stopped_in_spec(&self, media_type: TrackMediaType) -> bool {
        match media_type {
            TrackMediaType::Audio => self.tracks_spec.audio_recv == 0,
            TrackMediaType::Video => self.tracks_spec.video_recv == 0,
        }
    }

    /// Removes [`TrackStat`] with [`Direction::Send`].
    fn remove_sender(&mut self, sender_id: &StatId) {
        debug!(
            "Sender TrackStat [id = {:?}, peer_id = {}] was removed.",
            sender_id, self.peer_id
        );
        self.senders.remove(sender_id);
    }

    /// Removes [`TrackStat`] with [`Direction::Recv`].
    fn remove_receiver(&mut self, receiver_id: &StatId) {
        debug!(
            "Receiver TrackStat [id = {:?}, peer_id = {}] was removed.",
            receiver_id, self.peer_id
        );
        self.receivers.remove(receiver_id);
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

    /// Removes [`TrackStat`]s which was considered as stopped.
    #[allow(clippy::if_not_else)]
    fn remove_stopped_stats(&mut self) {
        let remove_senders: HashSet<_> = self
            .senders
            .iter()
            .filter_map(|(sender_id, sender)| {
                if !sender.is_flowing() {
                    Some(sender_id.clone())
                } else {
                    None
                }
            })
            .collect();
        let remove_receivers: HashSet<_> = self
            .receivers
            .iter()
            .filter_map(|(receiver_id, receiver)| {
                if !receiver.is_flowing() {
                    Some(receiver_id.clone())
                } else {
                    None
                }
            })
            .collect();

        remove_senders.iter().for_each(|sender_id| {
            self.remove_sender(sender_id);
        });
        remove_receivers.iter().for_each(|receiver_id| {
            self.remove_receiver(receiver_id);
        });
    }

    /// Returns [`MediaDirection`]s and [`MediaType`]s of the `MediaTrack`s
    /// which are currently is stopped.
    ///
    /// This is determined by comparing count of senders/receivers from the
    /// [`PeerSpec`].
    ///
    /// Also media type of sender/receiver and activity taken into account.
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

        match audio_send.cmp(&self.tracks_spec.audio_send) {
            Ordering::Less => self.send_traffic_state.disable(MediaType::Audio),
            Ordering::Equal => {
                if self.tracks_spec.audio_send > 0 {
                    self.send_traffic_state.started(MediaType::Audio)
                }
            }
            _ => (),
        }
        match video_send.cmp(&self.tracks_spec.video_send) {
            Ordering::Less => self.send_traffic_state.disable(MediaType::Video),
            Ordering::Equal => {
                if self.tracks_spec.video_send > 0 {
                    self.send_traffic_state.started(MediaType::Video)
                }
            }
            _ => (),
        }
        match audio_recv.cmp(&self.tracks_spec.audio_recv) {
            Ordering::Less => self.recv_traffic_state.disable(MediaType::Audio),
            Ordering::Equal => {
                if self.tracks_spec.audio_recv > 0 {
                    self.recv_traffic_state.started(MediaType::Audio)
                }
            }
            _ => (),
        }
        match video_recv.cmp(&self.tracks_spec.video_recv) {
            Ordering::Less => self.recv_traffic_state.disable(MediaType::Video),
            Ordering::Equal => {
                if self.tracks_spec.video_recv > 0 {
                    self.recv_traffic_state.started(MediaType::Video)
                }
            }
            _ => (),
        }
    }

    /// Returns `true` if all senders and receivers is not sending or receiving
    /// anything.
    fn is_stopped(&self) -> bool {
        self.recv_traffic_state.is_disabled(MediaType::Both)
            && self.send_traffic_state.is_disabled(MediaType::Both)
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

// TODO: unit tests
