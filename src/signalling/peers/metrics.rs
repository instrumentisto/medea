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
        for peer in self.peers.values() {
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
        if sender.is_flowing() {
            let sender_media_type: MediaType = sender.media_type.into();
            self.send_traffic_state.started(sender_media_type);
        }
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
        if receiver.is_flowing() {
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

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::Arc,
        time::{Duration, Instant, SystemTime},
    };

    use futures::{channel::mpsc, stream::LocalBoxStream, StreamExt as _};
    use medea_client_api_proto::{
        stats::{
            HighResTimeStamp, MediaSourceKind, RtcInboundRtpStreamMediaType,
            RtcInboundRtpStreamStats, RtcOutboundRtpStreamMediaType,
            RtcOutboundRtpStreamStats, RtcStat, RtcStatsType, StatId,
        },
        PeerId,
    };
    use tokio::time::delay_for;

    use crate::{
        api::control::callback::{MediaDirection, MediaType},
        media::peer::{
            tests::test_peer_from_peer_tracks, Context, Peer, Stable,
        },
        signalling::peers::{
            traffic_watcher::MockPeerTrafficWatcher, PeersMetricsEvent,
        },
        utils::test::{timestamp, wait_or_fail},
    };

    use super::{PeerTracks, PeersMetricsService};
    use std::collections::HashSet;

    fn outbound_traffic(
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

    fn inbound_traffic(
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

    struct Helper {
        traffic_flows_stream: LocalBoxStream<'static, ()>,
        traffic_stopped_stream: LocalBoxStream<'static, ()>,
        peer_events_stream: LocalBoxStream<'static, PeersMetricsEvent>,
        metrics: PeersMetricsService,
    }

    impl Helper {
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
            let mut traffic_flows_stream = Box::pin(traffic_flows_rx);
            watcher.expect_traffic_flows().returning(move |_, _, _| {
                traffic_flows_tx.unbounded_send(()).unwrap();
            });
            let (traffic_stopped_tx, traffic_stopped_rx) = mpsc::unbounded();
            let mut traffic_stopped_stream = Box::pin(traffic_stopped_rx);
            watcher.expect_traffic_stopped().returning(move |_, _, _| {
                traffic_stopped_tx.unbounded_send(()).unwrap();
            });

            let mut metrics = PeersMetricsService::new(
                "test".to_string().into(),
                Arc::new(watcher),
            );

            Self {
                traffic_flows_stream,
                traffic_stopped_stream,
                peer_events_stream: Box::pin(metrics.subscribe()),
                metrics,
            }
        }

        pub fn register_peer(
            &mut self,
            send_audio: u32,
            send_video: u32,
            recv_audio: u32,
            recv_video: u32,
        ) {
            self.metrics.register_peer(
                &test_peer_from_peer_tracks(
                    send_audio, send_video, recv_audio, recv_video,
                ),
                Duration::from_millis(50),
            );
        }

        pub fn start_traffic_flowing(
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
                    timestamp: timestamp(SystemTime::now()),
                    stats: RtcStatsType::OutboundRtp(Box::new(
                        outbound_traffic(packets, true),
                    )),
                });
            }

            for i in 0..send_video {
                stats.push(RtcStat {
                    id: StatId(format!("{}-send-video", i)),
                    timestamp: timestamp(SystemTime::now()),
                    stats: RtcStatsType::OutboundRtp(Box::new(
                        outbound_traffic(packets, false),
                    )),
                })
            }

            for i in 0..recv_audio {
                stats.push(RtcStat {
                    id: StatId(format!("{}-recv-audio", i)),
                    timestamp: timestamp(SystemTime::now()),
                    stats: RtcStatsType::InboundRtp(Box::new(inbound_traffic(
                        packets, true,
                    ))),
                })
            }

            for i in 0..recv_video {
                stats.push(RtcStat {
                    id: StatId(format!("{}-recv-video", i)),
                    timestamp: timestamp(SystemTime::now()),
                    stats: RtcStatsType::InboundRtp(Box::new(inbound_traffic(
                        packets, false,
                    ))),
                })
            }

            self.metrics.add_stat(PeerId(1), stats);
        }

        pub async fn next_traffic_flows(&mut self) {
            self.traffic_flows_stream.next().await;
        }

        pub async fn next_traffic_stopped(&mut self) {
            self.metrics.check_peers();
            self.traffic_stopped_stream.next().await;
        }

        pub async fn next_event(&mut self) -> PeersMetricsEvent {
            self.peer_events_stream.next().await.unwrap()
        }

        pub fn check_peers(&mut self) {
            self.metrics.check_peers();
        }

        pub fn unregister_peer(&mut self, peer_id: PeerId) {
            self.metrics.unregister_peers(&[peer_id]);
        }
    }

    #[actix_rt::test]
    async fn traffic_flows_and_stopped_works() {
        let mut helper = Helper::new();
        helper.register_peer(1, 1, 1, 1);
        helper.start_traffic_flowing(1, 1, 1, 1, 100);
        helper.next_traffic_flows().await;

        delay_for(Duration::from_millis(50)).await;

        helper.next_traffic_stopped().await;
    }

    #[actix_rt::test]
    async fn no_traffic_event_works() {
        let mut helper = Helper::new();
        helper.register_peer(1, 1, 1, 1);
        helper.start_traffic_flowing(1, 1, 1, 1, 100);
        let _ = helper.next_event().await;
        let _ = helper.next_event().await;
        delay_for(Duration::from_millis(40)).await;
        helper.start_traffic_flowing(1, 1, 0, 0, 200);
        delay_for(Duration::from_millis(15)).await;
        helper.check_peers();

        loop {
            let event = helper.next_event().await;
            match event {
                PeersMetricsEvent::NoTrafficFlow {
                    peer_id,
                    direction,
                    media_type,
                    ..
                } => {
                    assert_eq!(peer_id, PeerId(1));
                    assert_eq!(direction, MediaDirection::Play);
                    assert_eq!(media_type, MediaType::Both);
                    break;
                }
                _ => panic!("Unexpected event received: {:?}.", event),
            }
        }

        helper.start_traffic_flowing(1, 1, 1, 1, 300);
        let event = helper.next_event().await;
        match event {
            PeersMetricsEvent::TrafficFlows {
                peer_id,
                direction,
                media_type,
            } => {
                assert_eq!(peer_id, PeerId(1));
                assert_eq!(direction, MediaDirection::Play);
                assert_eq!(media_type, MediaType::Both);
            }
            _ => panic!("Unexpected event received: {:?}.", event),
        }
    }

    #[actix_rt::test]
    async fn peer_unregistering_doesnt_trigger_anything() {
        let mut helper = Helper::new();
        helper.register_peer(1, 1, 1, 1);
        helper.start_traffic_flowing(1, 1, 1, 1, 100);

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
        wait_or_fail(helper.next_event(), Duration::from_millis(10))
            .await
            .unwrap_err();
        wait_or_fail(helper.next_traffic_stopped(), Duration::from_millis(10))
            .await
            .unwrap_err();
    }
}
