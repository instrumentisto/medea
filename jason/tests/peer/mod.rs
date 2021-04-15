#![cfg(target_arch = "wasm32")]

mod media;

use std::{pin::Pin, rc::Rc};

use futures::{
    channel::mpsc,
    future::{self, FutureExt as _},
    Stream, StreamExt as _,
};
use medea_client_api_proto::{
    stats::{
        HighResTimeStamp, KnownIceCandidatePairState, NonExhaustive,
        RtcInboundRtpStreamMediaType, RtcOutboundRtpStreamMediaType, RtcStat,
        RtcStatsType, StatId, TrackStats, TrackStatsKind,
    },
    AudioSettings, Direction, IceConnectionState, MediaSourceKind, MediaType,
    MemberId, NegotiationRole, PeerId, Track, TrackId, TrackPatchEvent,
    VideoSettings,
};
use medea_jason::{
    connection::Connections,
    media::{LocalTracksConstraints, MediaKind, MediaManager, RecvConstraints},
    peer::{
        self, media_exchange_state, MediaStateControllable, PeerEvent,
        TrackDirection,
    },
    platform::RtcStats,
    utils::Updatable,
};
use wasm_bindgen_test::*;

use crate::{
    delay_for, get_media_stream_settings, get_test_recv_tracks,
    get_test_unrequired_tracks, local_constraints, timeout,
};

wasm_bindgen_test_configure!(run_in_browser);

#[inline]
#[must_use]
fn toggle_disable_track_update(id: TrackId, enabled: bool) -> TrackPatchEvent {
    TrackPatchEvent {
        id,
        enabled_individual: Some(enabled),
        enabled_general: Some(enabled),
        muted: None,
    }
}

const AUDIO_TRACK_ID: TrackId = TrackId(1);
const VIDEO_TRACK_ID: TrackId = TrackId(2);

#[wasm_bindgen_test]
async fn disable_enable_audio() {
    let (tx, _rx) = mpsc::unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_unrequired_tracks();
    let peer_state = peer::State::new(
        PeerId(1),
        Vec::new(),
        false,
        Some(NegotiationRole::Offerer),
    );
    let send_constraints = local_constraints(true, true);
    let peer = peer::Component::new(
        peer::PeerConnection::new(
            &peer_state,
            tx,
            manager,
            send_constraints.clone(),
            Rc::new(Connections::default()),
            Rc::new(RecvConstraints::default()),
        )
        .unwrap(),
        Rc::new(peer_state),
    );

    peer.state()
        .insert_track(&audio_track, send_constraints.clone())
        .unwrap();
    peer.state()
        .insert_track(&video_track, send_constraints.clone())
        .unwrap();
    peer.state().when_local_sdp_updated().await.unwrap();
    assert!(peer.is_send_audio_enabled());
    assert!(peer.is_send_video_enabled(None));

    peer.state()
        .patch_track(&toggle_disable_track_update(AUDIO_TRACK_ID, false));
    peer.state().when_updated().await;
    assert!(!peer.is_send_audio_enabled());
    assert!(peer.is_send_video_enabled(None));

    peer.state()
        .patch_track(&toggle_disable_track_update(AUDIO_TRACK_ID, true));
    peer.state().when_updated().await;
    assert!(peer.is_send_audio_enabled());
    assert!(peer.is_send_video_enabled(None));
}

#[wasm_bindgen_test]
async fn disable_enable_video() {
    let (tx, _rx) = mpsc::unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_unrequired_tracks();

    let peer_state = peer::State::new(
        PeerId(1),
        Vec::new(),
        false,
        Some(NegotiationRole::Offerer),
    );
    let send_constraints = local_constraints(true, true);
    let peer = peer::Component::new(
        peer::PeerConnection::new(
            &peer_state,
            tx,
            manager,
            send_constraints.clone(),
            Rc::new(Connections::default()),
            Rc::new(RecvConstraints::default()),
        )
        .unwrap(),
        Rc::new(peer_state),
    );
    peer.state()
        .insert_track(&audio_track, send_constraints.clone())
        .unwrap();
    peer.state()
        .insert_track(&video_track, send_constraints.clone())
        .unwrap();
    peer.state().when_local_sdp_updated().await.unwrap();

    assert!(peer.is_send_audio_enabled());
    assert!(peer.is_send_video_enabled(None));

    peer.state()
        .patch_track(&toggle_disable_track_update(VIDEO_TRACK_ID, false));
    peer.state().when_updated().await;
    assert!(peer.is_send_audio_enabled());
    assert!(!peer.is_send_video_enabled(None));

    peer.state()
        .patch_track(&toggle_disable_track_update(VIDEO_TRACK_ID, true));
    peer.state().when_updated().await;
    assert!(peer.is_send_audio_enabled());
    assert!(peer.is_send_video_enabled(None));
}

#[wasm_bindgen_test]
async fn new_with_disable_audio() {
    let (tx, _rx) = mpsc::unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_unrequired_tracks();
    let peer_state = peer::State::new(PeerId(1), Vec::new(), false, None);
    let send_constraints = local_constraints(false, true);
    peer_state
        .insert_track(&audio_track, send_constraints.clone())
        .unwrap();
    peer_state
        .insert_track(&video_track, send_constraints.clone())
        .unwrap();
    let peer = peer::Component::new(
        peer::PeerConnection::new(
            &peer_state,
            tx,
            manager,
            send_constraints.clone(),
            Rc::new(Connections::default()),
            Rc::new(RecvConstraints::default()),
        )
        .unwrap(),
        Rc::new(peer_state),
    );
    peer.state().when_all_updated().await;

    assert!(!peer.is_send_audio_enabled());
    assert!(peer.is_send_video_enabled(None));
}

#[wasm_bindgen_test]
async fn new_with_disable_video() {
    let (tx, _rx) = mpsc::unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_unrequired_tracks();
    let peer_state = peer::State::new(PeerId(1), Vec::new(), false, None);
    let send_constraints = local_constraints(true, false);
    let peer = peer::Component::new(
        peer::PeerConnection::new(
            &peer_state,
            tx,
            manager,
            send_constraints.clone(),
            Rc::new(Connections::default()),
            Rc::new(RecvConstraints::default()),
        )
        .unwrap(),
        Rc::new(peer_state),
    );
    peer.state()
        .insert_track(&audio_track, send_constraints.clone())
        .unwrap();
    peer.state()
        .insert_track(&video_track, send_constraints.clone())
        .unwrap();
    peer.state().when_all_updated().await;

    assert!(peer.is_send_audio_enabled());
    assert!(!peer.is_send_video_enabled(None));
}

#[wasm_bindgen_test]
async fn add_candidates_to_answerer_before_offer() {
    let (tx1, rx1) = mpsc::unbounded();
    let (tx2, _) = mpsc::unbounded();
    let (audio_track, video_track) = get_test_unrequired_tracks();

    let manager = Rc::new(MediaManager::default());
    let pc1_state = peer::State::new(
        PeerId(1),
        Vec::new(),
        false,
        Some(NegotiationRole::Offerer),
    );
    let pc1 = peer::Component::new(
        peer::PeerConnection::new(
            &pc1_state,
            tx1,
            Rc::clone(&manager),
            LocalTracksConstraints::default(),
            Rc::new(Connections::default()),
            Rc::new(RecvConstraints::default()),
        )
        .unwrap(),
        Rc::new(pc1_state),
    );
    pc1.state()
        .insert_track(&audio_track, LocalTracksConstraints::default())
        .unwrap();
    pc1.state()
        .insert_track(&video_track, LocalTracksConstraints::default())
        .unwrap();
    let pc1_offer = pc1.state().when_local_sdp_updated().await.unwrap();

    let pc2_state = peer::State::new(PeerId(2), Vec::new(), false, None);
    let pc2 = peer::Component::new(
        peer::PeerConnection::new(
            &pc2_state,
            tx2,
            Rc::clone(&manager),
            LocalTracksConstraints::default(),
            Rc::new(Connections::default()),
            Rc::new(RecvConstraints::default()),
        )
        .unwrap(),
        Rc::new(pc2_state),
    );

    handle_ice_candidates(rx1, &pc2, 1).await;
    assert!(pc2.candidates_buffer_len() > 0);

    pc2.state()
        .set_negotiation_role(NegotiationRole::Answerer(pc1_offer))
        .await;
    pc2.state().when_local_sdp_updated().await.unwrap();
    assert_eq!(pc2.candidates_buffer_len(), 0);
}

#[wasm_bindgen_test]
async fn add_candidates_to_offerer_before_answer() {
    let (tx1, _) = mpsc::unbounded();
    let (tx2, rx2) = mpsc::unbounded();
    let (audio_track, video_track) = get_test_unrequired_tracks();

    let manager = Rc::new(MediaManager::default());
    let pc1_state = peer::State::new(
        PeerId(1),
        Vec::new(),
        false,
        Some(NegotiationRole::Offerer),
    );
    let pc1 = peer::Component::new(
        peer::PeerConnection::new(
            &pc1_state,
            tx1,
            Rc::clone(&manager),
            LocalTracksConstraints::default(),
            Rc::new(Connections::default()),
            Rc::new(RecvConstraints::default()),
        )
        .unwrap(),
        Rc::new(pc1_state),
    );
    pc1.state()
        .insert_track(&audio_track, LocalTracksConstraints::default())
        .unwrap();
    pc1.state()
        .insert_track(&video_track, LocalTracksConstraints::default())
        .unwrap();

    let pc2_state = peer::State::new(PeerId(2), Vec::new(), false, None);
    let pc2 = peer::Component::new(
        peer::PeerConnection::new(
            &pc2_state,
            tx2,
            Rc::clone(&manager),
            LocalTracksConstraints::default(),
            Rc::new(Connections::default()),
            Rc::new(RecvConstraints::default()),
        )
        .unwrap(),
        Rc::new(pc2_state),
    );

    let offer = pc1.state().when_local_sdp_updated().await.unwrap();
    pc2.state()
        .set_negotiation_role(NegotiationRole::Answerer(offer))
        .await;
    let answer = pc2.state().when_local_sdp_updated().await.unwrap();

    handle_ice_candidates(rx2, &pc1, 1).await;

    // assert that pc1 has buffered candidates
    assert!(pc1.candidates_buffer_len() > 0);
    pc1.state().set_remote_sdp(answer);
    pc1.state().when_remote_sdp_processed().await;
    // assert that pc1 has buffered candidates got fulshed
    assert_eq!(pc1.candidates_buffer_len(), 0);
}

#[wasm_bindgen_test]
async fn normal_exchange_of_candidates() {
    let (tx1, rx1) = mpsc::unbounded();
    let (tx2, rx2) = mpsc::unbounded();

    let (audio_track, video_track) = get_test_unrequired_tracks();

    let manager = Rc::new(MediaManager::default());
    let pc1_state = peer::State::new(
        PeerId(1),
        Vec::new(),
        false,
        Some(NegotiationRole::Offerer),
    );
    let pc1 = peer::Component::new(
        peer::PeerConnection::new(
            &pc1_state,
            tx1,
            Rc::clone(&manager),
            LocalTracksConstraints::default(),
            Rc::new(Connections::default()),
            Rc::new(RecvConstraints::default()),
        )
        .unwrap(),
        Rc::new(pc1_state),
    );
    pc1.state()
        .insert_track(&audio_track, LocalTracksConstraints::default())
        .unwrap();
    pc1.state()
        .insert_track(&video_track, LocalTracksConstraints::default())
        .unwrap();

    let pc2_state = peer::State::new(PeerId(2), Vec::new(), false, None);
    let pc2 = peer::Component::new(
        peer::PeerConnection::new(
            &pc2_state,
            tx2,
            Rc::clone(&manager),
            LocalTracksConstraints::default(),
            Rc::new(Connections::default()),
            Rc::new(RecvConstraints::default()),
        )
        .unwrap(),
        Rc::new(pc2_state),
    );

    let offer = pc1.state().when_local_sdp_updated().await.unwrap();
    pc2.state()
        .set_negotiation_role(NegotiationRole::Answerer(offer))
        .await;
    let answer = pc2.state().when_local_sdp_updated().await.unwrap();
    pc1.state().set_remote_sdp(answer);
    pc1.state().when_remote_sdp_processed().await;

    handle_ice_candidates(rx1, &pc2, 1).await;
    handle_ice_candidates(rx2, &pc1, 1).await;
}

async fn handle_ice_candidates(
    mut candidates_rx: mpsc::UnboundedReceiver<PeerEvent>,
    peer: &peer::Component,
    count: u8,
) {
    let mut cnt = 0;

    while let Some(event) = candidates_rx.next().await {
        match event {
            PeerEvent::IceCandidateDiscovered {
                peer_id: _,
                candidate,
                sdp_m_line_index,
                sdp_mid,
            } => {
                peer.add_ice_candidate(candidate, sdp_m_line_index, sdp_mid)
                    .await
                    .unwrap();

                cnt += 1;
                if cnt == count {
                    break;
                }
            }
            _ => (),
        }
    }
}

#[wasm_bindgen_test]
async fn send_event_on_new_local_stream() {
    let (tx, mut rx) = mpsc::unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_unrequired_tracks();
    let send_constraints: LocalTracksConstraints =
        get_media_stream_settings(true, true).into();

    let peer_state = peer::State::new(
        PeerId(1),
        Vec::new(),
        false,
        Some(NegotiationRole::Offerer),
    );
    peer_state
        .insert_track(&audio_track, send_constraints.clone())
        .unwrap();
    peer_state
        .insert_track(&video_track, send_constraints.clone())
        .unwrap();
    let peer = peer::Component::new(
        peer::PeerConnection::new(
            &peer_state,
            tx,
            manager,
            send_constraints.clone(),
            Rc::new(Connections::default()),
            Rc::new(RecvConstraints::default()),
        )
        .unwrap(),
        Rc::new(peer_state),
    );
    peer.state().when_local_sdp_updated().await.unwrap();

    while let Some(event) = rx.next().await {
        match event {
            PeerEvent::NewLocalTrack { .. } => {
                break;
            }
            _ => {}
        }
    }
}

/// Setup signalling between two peers and wait for:
/// 1. `IceConnectionState::Checking` from both peers.
/// 2. `IceConnectionState::Connected` from both peers.
#[wasm_bindgen_test]
async fn ice_connection_state_changed_is_emitted() {
    let (tx1, rx1) = mpsc::unbounded();
    let (tx2, rx2) = mpsc::unbounded();
    let (audio_track, video_track) = get_test_unrequired_tracks();

    let manager = Rc::new(MediaManager::default());
    let pc1_state = peer::State::new(
        PeerId(1),
        Vec::new(),
        false,
        Some(NegotiationRole::Offerer),
    );
    let pc1 = peer::Component::new(
        peer::PeerConnection::new(
            &pc1_state,
            tx1,
            manager.clone(),
            LocalTracksConstraints::default(),
            Rc::new(Connections::default()),
            Rc::new(RecvConstraints::default()),
        )
        .unwrap(),
        Rc::new(pc1_state),
    );
    pc1.state()
        .insert_track(&audio_track, LocalTracksConstraints::default())
        .unwrap();
    pc1.state()
        .insert_track(&video_track, LocalTracksConstraints::default())
        .unwrap();
    let pc1_offer = pc1.state().when_local_sdp_updated().await.unwrap();

    let pc2_state = peer::State::new(
        PeerId(2),
        Vec::new(),
        false,
        Some(NegotiationRole::Answerer(pc1_offer)),
    );
    let pc2 = peer::Component::new(
        peer::PeerConnection::new(
            &pc2_state,
            tx2,
            manager,
            LocalTracksConstraints::default(),
            Rc::new(Connections::default()),
            Rc::new(RecvConstraints::default()),
        )
        .unwrap(),
        Rc::new(pc2_state),
    );

    let answer = pc2.state().when_local_sdp_updated().await.unwrap();
    pc1.state().set_remote_sdp(answer);
    pc1.state().when_remote_sdp_processed().await;

    let mut events = futures::stream::select(rx1, rx2);

    let mut checking1 = false;
    let mut checking2 = false;
    let mut connected1 = false;
    let mut connected2 = false;
    while let Some(event) = events.next().await {
        let event: PeerEvent = event;
        match event {
            PeerEvent::IceCandidateDiscovered {
                peer_id,
                candidate,
                sdp_m_line_index,
                sdp_mid,
            } => {
                if peer_id.0 == 1 {
                    pc2.add_ice_candidate(candidate, sdp_m_line_index, sdp_mid)
                        .await
                        .unwrap();
                } else {
                    pc1.add_ice_candidate(candidate, sdp_m_line_index, sdp_mid)
                        .await
                        .unwrap();
                }
            }
            PeerEvent::IceConnectionStateChanged {
                peer_id,
                ice_connection_state,
            } => match ice_connection_state {
                IceConnectionState::Checking => {
                    if peer_id.0 == 1 {
                        checking1 = true;
                    } else {
                        checking2 = true;
                    }
                }
                IceConnectionState::Connected => {
                    if peer_id.0 == 1 {
                        connected1 = true;
                    } else {
                        connected2 = true;
                    }
                }
                _ => {}
            },
            _ => {}
        };

        if checking1 && checking2 && connected1 && connected2 {
            break;
        }
    }
}
/// Two interconnected [`PeerConnection`]s for the test purposes.
///
/// `first_peer`
struct InterconnectedPeers {
    /// This [`PeerConnection`] will have one `video` track with `send`
    /// direction and one `audio` track with `send` direction.
    pub first_peer: peer::Component,

    /// This [`PeerConnection`] will have one `video` track with `recv`
    /// direction and one `audio` track with `recv` direction.
    pub second_peer: peer::Component,

    /// All [`PeerEvent`]s of this two interconnected [`PeerConnection`]s.
    pub peer_events_recv: Pin<Box<dyn Stream<Item = PeerEvent>>>,
}

impl InterconnectedPeers {
    /// Creates new interconnected [`PeerConnection`]s.
    pub async fn new() -> Self {
        let (tx1, peer_events_stream1) = mpsc::unbounded();
        let (tx2, peer_events_stream2) = mpsc::unbounded();

        let pc1_send_cons = local_constraints(true, true);
        let manager = Rc::new(MediaManager::default());
        let pc1_state = peer::State::new(
            PeerId(1),
            Vec::new(),
            false,
            Some(NegotiationRole::Offerer),
        );
        for track in Self::get_peer1_tracks() {
            pc1_state
                .insert_track(&track, pc1_send_cons.clone())
                .unwrap();
        }
        let pc1 = peer::Component::new(
            peer::PeerConnection::new(
                &pc1_state,
                tx1,
                manager.clone(),
                pc1_send_cons,
                Rc::new(Connections::default()),
                Rc::new(RecvConstraints::default()),
            )
            .unwrap(),
            Rc::new(pc1_state),
        );

        let pc1_offer = pc1.state().when_local_sdp_updated().await.unwrap();
        let pc2_send_cons = local_constraints(true, true);
        let pc2_state = peer::State::new(
            PeerId(2),
            Vec::new(),
            false,
            Some(NegotiationRole::Answerer(pc1_offer)),
        );
        for track in Self::get_peer2_tracks() {
            pc2_state
                .insert_track(&track, pc2_send_cons.clone())
                .unwrap();
        }
        let pc2 = peer::Component::new(
            peer::PeerConnection::new(
                &pc2_state,
                tx2,
                manager,
                pc2_send_cons,
                Rc::new(Connections::default()),
                Rc::new(RecvConstraints::default()),
            )
            .unwrap(),
            Rc::new(pc2_state),
        );

        let pc2_offer = pc2.state().when_local_sdp_updated().await.unwrap();
        pc1.state().set_remote_sdp(pc2_offer);
        pc1.state().when_remote_sdp_processed().await;

        let events =
            futures::stream::select(peer_events_stream1, peer_events_stream2);

        let mut interconnected_peers = Self {
            first_peer: pc1,
            second_peer: pc2,
            peer_events_recv: Box::pin(events),
        };

        interconnected_peers.handle_ice_candidates().await;

        interconnected_peers
    }

    /// Handles [`PeerEvent::IceCandidateDiscovered`] and
    /// [`PeerEvent::IceConnectionStateChange`] events.
    ///
    /// This [`Future`] will be resolved when all needed ICE candidates will
    /// received and [`PeerConnection`]'s ICE connection state will be
    /// transit into
    /// [`IceConnectionState::Connected`].
    async fn handle_ice_candidates(&mut self) {
        let mut checking1 = false;
        let mut checking2 = false;
        let mut connected1 = false;
        let mut connected2 = false;
        while let Some(event) = self.peer_events_recv.next().await {
            let event: PeerEvent = event;
            match event {
                PeerEvent::IceCandidateDiscovered {
                    peer_id,
                    candidate,
                    sdp_m_line_index,
                    sdp_mid,
                } => {
                    if peer_id.0 == 1 {
                        self.second_peer
                            .add_ice_candidate(
                                candidate,
                                sdp_m_line_index,
                                sdp_mid,
                            )
                            .await
                            .unwrap();
                    } else {
                        self.first_peer
                            .add_ice_candidate(
                                candidate,
                                sdp_m_line_index,
                                sdp_mid,
                            )
                            .await
                            .unwrap();
                    }
                }
                PeerEvent::IceConnectionStateChanged {
                    peer_id,
                    ice_connection_state,
                } => match ice_connection_state {
                    IceConnectionState::Checking => {
                        if peer_id.0 == 1 {
                            checking1 = true;
                        } else {
                            checking2 = true;
                        }
                    }
                    IceConnectionState::Connected => {
                        if peer_id.0 == 1 {
                            connected1 = true;
                        } else {
                            connected2 = true;
                        }
                    }
                    _ => {}
                },
                _ => {}
            };

            if checking1 && checking2 && connected1 && connected2 {
                break;
            }
        }
    }

    /// Returns [`Track`]s for the `first_peer`.
    fn get_peer1_tracks() -> Vec<Track> {
        vec![
            Track {
                id: TrackId(1),
                direction: Direction::Send {
                    receivers: vec![MemberId::from("bob")],
                    mid: None,
                },
                media_type: MediaType::Audio(AudioSettings { required: true }),
            },
            Track {
                id: TrackId(2),
                direction: Direction::Send {
                    receivers: vec![MemberId::from("bob")],
                    mid: None,
                },
                media_type: MediaType::Video(VideoSettings {
                    required: true,
                    source_kind: MediaSourceKind::Device,
                }),
            },
        ]
    }

    /// Returns [`Track`]s for the `second_peer`.
    fn get_peer2_tracks() -> Vec<Track> {
        vec![
            Track {
                id: TrackId(1),
                direction: Direction::Recv {
                    sender: MemberId::from("alice"),
                    mid: None,
                },
                media_type: MediaType::Audio(AudioSettings { required: true }),
            },
            Track {
                id: TrackId(2),
                direction: Direction::Recv {
                    sender: MemberId::from("alice"),
                    mid: None,
                },
                media_type: MediaType::Video(VideoSettings {
                    required: true,
                    source_kind: MediaSourceKind::Device,
                }),
            },
        ]
    }
}

/// Tests that [`PeerConnection::get_stats`] works correctly and provides stats
/// which we need at the moment.
#[wasm_bindgen_test]
async fn get_traffic_stats() {
    let peers = InterconnectedPeers::new().await;

    let first_peer_stats = peers.first_peer.get_stats().await.unwrap();
    let mut first_peer_video_outbound_stats_count = 0;
    let mut first_peer_audio_outbound_stats_count = 0;
    for stat in first_peer_stats.0 {
        match stat.stats {
            RtcStatsType::OutboundRtp(outbound) => match outbound.media_type {
                RtcOutboundRtpStreamMediaType::Audio { .. } => {
                    first_peer_audio_outbound_stats_count += 1
                }
                RtcOutboundRtpStreamMediaType::Video { .. } => {
                    first_peer_video_outbound_stats_count += 1
                }
            },
            RtcStatsType::InboundRtp(_) => {
                unreachable!(
                    "First Peer shouldn't have any InboundRtp
stats."
                )
            }
            RtcStatsType::CandidatePair(candidate_pair) => {
                assert_eq!(
                    candidate_pair.state,
                    NonExhaustive::Known(KnownIceCandidatePairState::Succeeded)
                );
            }
            _ => (),
        }
    }
    assert_eq!(first_peer_video_outbound_stats_count, 1);
    assert_eq!(first_peer_audio_outbound_stats_count, 1);

    let second_peer_stats = peers.second_peer.get_stats().await.unwrap();
    let mut second_peer_video_inbound_stats_count = 0;
    let mut second_peer_audio_inbound_stats_count = 0;
    let mut has_succeeded_pair = false;
    for stat in second_peer_stats.0 {
        match stat.stats {
            RtcStatsType::InboundRtp(inbound) => {
                match inbound.media_specific_stats {
                    RtcInboundRtpStreamMediaType::Audio { .. } => {
                        second_peer_audio_inbound_stats_count += 1
                    }
                    RtcInboundRtpStreamMediaType::Video { .. } => {
                        second_peer_video_inbound_stats_count += 1
                    }
                }
            }
            RtcStatsType::OutboundRtp(_) => unreachable!(
                "Second Peer shouldn't have any OutboundRtp stats."
            ),
            RtcStatsType::CandidatePair(candidate_pair) => {
                if let NonExhaustive::Known(
                    KnownIceCandidatePairState::Succeeded,
                ) = candidate_pair.state
                {
                    has_succeeded_pair = true;
                }
            }
            _ => (),
        }
    }
    assert!(has_succeeded_pair);
    assert_eq!(second_peer_video_inbound_stats_count, 1);
    assert_eq!(second_peer_audio_inbound_stats_count, 1);
}

/// Tests for a [`RtcStat`]s caching mechanism of the [`PeerConnection`].
mod peer_stats_caching {
    use super::*;

    /// Tests that [`PeerConnection::send_peer_stats`] will send only one
    /// [`RtcStat`] update when we try to send two identical [`RtcStat`]s.
    #[wasm_bindgen_test]
    async fn works() {
        let (tx, peer_events_stream) = mpsc::unbounded();
        let manager = Rc::new(MediaManager::default());
        let peer_state = peer::State::new(PeerId(1), Vec::new(), false, None);
        let peer = peer::Component::new(
            peer::PeerConnection::new(
                &peer_state,
                tx,
                manager,
                LocalTracksConstraints::default(),
                Rc::new(Connections::default()),
                Rc::new(RecvConstraints::default()),
            )
            .unwrap(),
            Rc::new(peer_state),
        );

        let stat = RtcStat {
            id: StatId("2ef2e34c".to_string()),
            timestamp: HighResTimeStamp(1584373509700.0),
            stats: RtcStatsType::Track(Box::new(TrackStats {
                track_identifier: "0d4f8e05-51d8-4f9b-90b2-453401fc8041"
                    .to_string(),
                kind: Some(TrackStatsKind::Audio),
                remote_source: None,
                ended: Some(false),
            })),
        };
        peer.send_peer_stats(RtcStats(vec![stat.clone()]));

        let mut peer_events_stream = peer_events_stream.filter_map(|event| {
            Box::pin(async move {
                if let PeerEvent::StatsUpdate { peer_id: _, stats } = event {
                    Some(stats)
                } else {
                    None
                }
            })
        });
        let first_rtc_stats = peer_events_stream.next().await.unwrap();
        assert_eq!(first_rtc_stats.0[0], stat);

        peer.send_peer_stats(RtcStats(vec![stat]));
        timeout(100, peer_events_stream.next()).await.unwrap_err();
    }

    /// Tests that [`PeerConnection::send_peer_stats`] will send two
    /// [`RtcStat`]s updates with identical content but with different
    /// [`StatId`]s.
    #[wasm_bindgen_test]
    async fn takes_into_account_stat_id() {
        let (tx, peer_events_stream) = mpsc::unbounded();
        let manager = Rc::new(MediaManager::default());
        let peer_state = peer::State::new(PeerId(1), Vec::new(), false, None);
        let peer = peer::Component::new(
            peer::PeerConnection::new(
                &peer_state,
                tx,
                manager,
                LocalTracksConstraints::default(),
                Rc::new(Connections::default()),
                Rc::new(RecvConstraints::default()),
            )
            .unwrap(),
            Rc::new(peer_state),
        );

        let mut stat = RtcStat {
            id: StatId("2ef2e34c".to_string()),
            timestamp: HighResTimeStamp(1584373509700.0),
            stats: RtcStatsType::Track(Box::new(TrackStats {
                track_identifier: "0d4f8e05-51d8-4f9b-90b2-453401fc8041"
                    .to_string(),
                kind: Some(TrackStatsKind::Audio),
                remote_source: None,
                ended: Some(false),
            })),
        };
        peer.send_peer_stats(RtcStats(vec![stat.clone()]));

        let mut peer_events_stream = peer_events_stream.filter_map(|event| {
            Box::pin(async move {
                if let PeerEvent::StatsUpdate { peer_id: _, stats } = event {
                    Some(stats)
                } else {
                    None
                }
            })
        });
        let first_rtc_stats = peer_events_stream.next().await.unwrap();
        assert_eq!(first_rtc_stats.0[0], stat);

        stat.id = StatId("3df3d34c".to_string());
        peer.send_peer_stats(RtcStats(vec![stat.clone()]));
        let first_rtc_stats = peer_events_stream.next().await.unwrap();
        assert_eq!(first_rtc_stats.0[0], stat);
    }

    /// Tests that [`PeerConnection::send_peer_stats`] will send two
    /// [`RtcStat`]s updates with different content, but with identical
    /// [`StatId`].
    #[wasm_bindgen_test]
    async fn sends_updated_stats() {
        let (tx, peer_events_stream) = mpsc::unbounded();
        let manager = Rc::new(MediaManager::default());
        let peer_state = peer::State::new(PeerId(1), Vec::new(), false, None);
        let peer = peer::Component::new(
            peer::PeerConnection::new(
                &peer_state,
                tx,
                manager,
                LocalTracksConstraints::default(),
                Rc::new(Connections::default()),
                Rc::new(RecvConstraints::default()),
            )
            .unwrap(),
            Rc::new(peer_state),
        );

        let mut track_stat = Box::new(TrackStats {
            track_identifier: "0d4f8e05-51d8-4f9b-90b2-453401fc8041"
                .to_string(),
            kind: Some(TrackStatsKind::Audio),
            remote_source: None,
            ended: Some(false),
        });
        let mut stat = RtcStat {
            id: StatId("2ef2e34c".to_string()),
            timestamp: HighResTimeStamp(1584373509700.0),
            stats: RtcStatsType::Track(track_stat.clone()),
        };
        peer.send_peer_stats(RtcStats(vec![stat.clone()]));

        let mut peer_events_stream = peer_events_stream.filter_map(|event| {
            Box::pin(async move {
                if let PeerEvent::StatsUpdate { peer_id: _, stats } = event {
                    Some(stats)
                } else {
                    None
                }
            })
        });
        let first_rtc_stats = peer_events_stream.next().await.unwrap();
        assert_eq!(first_rtc_stats.0[0], stat);

        track_stat.ended = Some(true);
        stat.stats = RtcStatsType::Track(track_stat);
        peer.send_peer_stats(RtcStats(vec![stat.clone()]));
        let first_rtc_stats = peer_events_stream.next().await.unwrap();
        assert_eq!(first_rtc_stats.0[0], stat);
    }
}

#[wasm_bindgen_test]
async fn reset_transition_timers() {
    let (tx, _) = mpsc::unbounded();
    let manager = Rc::new(MediaManager::default());

    let peer_state = peer::State::new(
        PeerId(1),
        Vec::new(),
        false,
        Some(NegotiationRole::Offerer),
    );
    let send_constraints = local_constraints(true, true);
    let recv_constraints = Rc::new(RecvConstraints::default());
    let (audio_tx, video_tx) = get_test_unrequired_tracks();
    let (audio_rx, video_rx) = get_test_recv_tracks();
    peer_state
        .insert_track(&audio_tx, send_constraints.clone())
        .unwrap();
    peer_state
        .insert_track(&audio_rx, send_constraints.clone())
        .unwrap();
    peer_state
        .insert_track(&video_tx, send_constraints.clone())
        .unwrap();
    peer_state
        .insert_track(&video_rx, send_constraints.clone())
        .unwrap();
    let peer = peer::Component::new(
        peer::PeerConnection::new(
            &peer_state,
            tx,
            manager,
            send_constraints,
            Rc::new(Connections::default()),
            recv_constraints,
        )
        .unwrap(),
        Rc::new(peer_state),
    );

    peer.state().when_local_sdp_updated().await.unwrap();

    let all_enabled = future::join_all(
        peer.get_transceivers_sides(
            MediaKind::Audio,
            TrackDirection::Send,
            None,
        )
        .into_iter()
        .chain(
            peer.get_transceivers_sides(
                MediaKind::Video,
                TrackDirection::Send,
                None,
            )
            .into_iter(),
        )
        .map(|s| {
            s.media_state_transition_to(
                media_exchange_state::Stable::Disabled.into(),
            )
            .unwrap();

            s.when_media_state_stable(
                media_exchange_state::Stable::Enabled.into(),
            )
        }),
    )
    .map(drop)
    .shared();

    delay_for(400).await;
    peer.state().connection_lost();
    timeout(600, all_enabled.clone()).await.unwrap_err();

    peer.state().connection_lost();
    delay_for(30).await;
    peer.state().synced();

    timeout(600, all_enabled).await.unwrap();
}

#[wasm_bindgen_test]
async fn new_remote_track() {
    #[derive(Debug, PartialEq)]
    struct FinalTrack {
        has_audio: bool,
        has_video: bool,
    }
    async fn helper(
        audio_tx_enabled: bool,
        video_tx_enabled: bool,
        audio_rx_enabled: bool,
        video_rx_enabled: bool,
    ) -> Result<FinalTrack, MediaKind> {
        let (tx1, _rx1) = mpsc::unbounded();
        let (tx2, mut rx2) = mpsc::unbounded();
        let manager = Rc::new(MediaManager::default());

        let tx_caps = LocalTracksConstraints::default();
        tx_caps.set_media_state(
            media_exchange_state::Stable::from(audio_tx_enabled).into(),
            MediaKind::Audio,
            None,
        );
        tx_caps.set_media_state(
            media_exchange_state::Stable::from(video_tx_enabled).into(),
            MediaKind::Video,
            None,
        );

        let sender_peer_state =
            peer::State::new(PeerId(1), Vec::new(), false, None);
        let sender_peer = peer::Component::new(
            peer::PeerConnection::new(
                &sender_peer_state,
                tx1,
                manager.clone(),
                tx_caps.clone(),
                Rc::new(Connections::default()),
                Rc::new(RecvConstraints::default()),
            )
            .unwrap(),
            Rc::new(sender_peer_state),
        );

        let (audio_track, video_track) = get_test_unrequired_tracks();
        sender_peer
            .state()
            .insert_track(&audio_track, tx_caps.clone())
            .unwrap();
        sender_peer
            .state()
            .insert_track(&video_track, tx_caps.clone())
            .unwrap();
        sender_peer
            .state()
            .set_negotiation_role(NegotiationRole::Offerer)
            .await;

        let sender_offer =
            sender_peer.state().when_local_sdp_updated().await.unwrap();

        let rcv_caps = RecvConstraints::default();
        rcv_caps.set_enabled(audio_rx_enabled, MediaKind::Audio);
        rcv_caps.set_enabled(video_rx_enabled, MediaKind::Video);

        let rcvr_peer_state =
            peer::State::new(PeerId(2), Vec::new(), false, None);
        let rcvr_peer = peer::Component::new(
            peer::PeerConnection::new(
                &rcvr_peer_state,
                tx2,
                manager,
                LocalTracksConstraints::default(),
                Rc::new(Connections::default()),
                Rc::new(rcv_caps),
            )
            .unwrap(),
            Rc::new(rcvr_peer_state),
        );

        rcvr_peer
            .state()
            .insert_track(
                &Track {
                    id: TrackId(1),
                    direction: Direction::Recv {
                        sender: MemberId::from("whatever"),
                        mid: Some(String::from("0")),
                    },
                    media_type: MediaType::Audio(AudioSettings {
                        required: true,
                    }),
                },
                LocalTracksConstraints::default(),
            )
            .unwrap();
        rcvr_peer
            .state()
            .insert_track(
                &Track {
                    id: TrackId(2),
                    direction: Direction::Recv {
                        sender: MemberId::from("whatever"),
                        mid: Some(String::from("1")),
                    },
                    media_type: MediaType::Video(VideoSettings {
                        required: true,
                        source_kind: MediaSourceKind::Device,
                    }),
                },
                LocalTracksConstraints::default(),
            )
            .unwrap();

        rcvr_peer.state().when_all_tracks_created().await;
        rcvr_peer.state().stabilize_all();
        rcvr_peer.state().when_all_updated().await;

        rcvr_peer
            .state()
            .set_negotiation_role(NegotiationRole::Answerer(sender_offer))
            .await;

        let answer = rcvr_peer.state().when_local_sdp_updated().await.unwrap();

        sender_peer.state().set_remote_sdp(answer);
        sender_peer.state().when_remote_sdp_processed().await;

        let mut result = FinalTrack {
            has_audio: false,
            has_video: false,
        };
        loop {
            match timeout(300, rx2.next()).await {
                Ok(Some(event)) => {
                    if let PeerEvent::NewRemoteTrack { track, .. } = event {
                        match track.kind() {
                            MediaKind::Audio => {
                                if result.has_audio {
                                    return Err(MediaKind::Audio);
                                } else {
                                    result.has_audio = true;
                                }
                            }
                            MediaKind::Video => {
                                if result.has_video {
                                    return Err(MediaKind::Video);
                                } else {
                                    result.has_video = true;
                                }
                            }
                        }
                    }
                }
                Ok(None) | Err(_) => {
                    break;
                }
            }
        }
        Ok(result)
    }

    fn bit_at(input: u32, n: u8) -> bool {
        (input >> n) & 1 != 0
    }

    for i in 0..16 {
        let audio_tx_enabled = bit_at(i, 0);
        let video_tx_enabled = bit_at(i, 1);
        let audio_rx_enabled = bit_at(i, 2);
        let video_rx_enabled = bit_at(i, 3);

        assert_eq!(
            helper(
                audio_tx_enabled,
                video_tx_enabled,
                audio_rx_enabled,
                video_rx_enabled
            )
            .await
            .unwrap(),
            FinalTrack {
                has_audio: audio_tx_enabled && audio_rx_enabled,
                has_video: video_tx_enabled && video_rx_enabled,
            },
            "{} {} {} {}",
            audio_tx_enabled,
            video_tx_enabled,
            audio_rx_enabled,
            video_rx_enabled,
        );
    }
}

mod ice_restart {
    use medea_jason::utils::{AsProtoState, SynchronizableState};

    use super::*;

    fn get_ice_pwds(offer: &str) -> Vec<&str> {
        offer
            .lines()
            .filter_map(|line| {
                if line.contains("ice-pwd") {
                    Some(line.split(':').skip(1).next().unwrap())
                } else {
                    None
                }
            })
            .collect()
    }

    fn get_ice_ufrags(offer: &str) -> Vec<&str> {
        offer
            .lines()
            .filter_map(|line| {
                if line.contains("ice-ufrag") {
                    Some(line.split(':').skip(1).next().unwrap())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Tests that after [`PeerConnection::restart_ice`] call, `ice-pwd` and
    /// `ice-ufrag` IDs will be updated in the SDP offer.
    #[wasm_bindgen_test]
    async fn ice_restart_works() {
        let peers = InterconnectedPeers::new().await;
        peers
            .first_peer
            .state()
            .set_negotiation_role(NegotiationRole::Offerer)
            .await;
        let sdp_offer_before = peers
            .first_peer
            .state()
            .when_local_sdp_updated()
            .await
            .unwrap();
        let ice_pwds_before = get_ice_pwds(&sdp_offer_before);
        let ice_ufrags_before = get_ice_ufrags(&sdp_offer_before);
        peers.first_peer.state().reset_negotiation_role();
        crate::delay_for(100).await;
        peers.first_peer.state().restart_ice();
        peers
            .first_peer
            .state()
            .set_negotiation_role(NegotiationRole::Offerer)
            .await;
        let sdp_offer_after = peers
            .first_peer
            .state()
            .when_local_sdp_updated()
            .await
            .unwrap();
        let ice_pwds_after = get_ice_pwds(&sdp_offer_after);
        let ice_ufrags_after = get_ice_ufrags(&sdp_offer_after);

        ice_pwds_before
            .into_iter()
            .zip(ice_pwds_after.into_iter())
            .for_each(|(before, after)| assert_ne!(before, after));
        ice_ufrags_before
            .into_iter()
            .zip(ice_ufrags_after.into_iter())
            .for_each(|(before, after)| assert_ne!(before, after));
    }

    /// Checks that ICE restart can be started by [`PeerState`] update.
    #[wasm_bindgen_test]
    async fn ice_restart_by_state() {
        let peers = InterconnectedPeers::new().await;
        peers
            .first_peer
            .state()
            .set_negotiation_role(NegotiationRole::Offerer)
            .await;
        let sdp_offer_before = peers
            .first_peer
            .state()
            .when_local_sdp_updated()
            .await
            .unwrap();
        let ice_pwds_before = get_ice_pwds(&sdp_offer_before);
        let ice_ufrags_before = get_ice_ufrags(&sdp_offer_before);

        peers
            .first_peer
            .state()
            .apply_local_sdp(sdp_offer_before.clone());
        peers.first_peer.state().reset_negotiation_role();
        delay_for(100).await;
        let mut proto_state = peers.first_peer.state().as_proto();
        proto_state.restart_ice = true;
        peers
            .first_peer
            .state()
            .apply(proto_state, &LocalTracksConstraints::default());

        peers
            .first_peer
            .state()
            .set_negotiation_role(NegotiationRole::Offerer)
            .await;
        let sdp_offer_after = peers
            .first_peer
            .state()
            .when_local_sdp_updated()
            .await
            .unwrap();
        let ice_pwds_after = get_ice_pwds(&sdp_offer_after);
        let ice_ufrags_after = get_ice_ufrags(&sdp_offer_after);

        ice_pwds_before
            .into_iter()
            .zip(ice_pwds_after.into_iter())
            .for_each(|(before, after)| assert_ne!(before, after));
        ice_ufrags_before
            .into_iter()
            .zip(ice_ufrags_after.into_iter())
            .for_each(|(before, after)| assert_ne!(before, after));
    }
}

/// Tests [`peer::State::patch_track`] method.
#[wasm_bindgen_test]
async fn disable_and_enable_all_tracks() {
    use media_exchange_state::Stable::{Disabled, Enabled};

    let (audio_track, video_track) = get_test_unrequired_tracks();
    let audio_track_id = audio_track.id;
    let video_track_id = video_track.id;
    let pc_state = peer::State::new(PeerId(0), Vec::new(), false, None);
    pc_state
        .insert_track(&audio_track, LocalTracksConstraints::default())
        .unwrap();
    pc_state
        .insert_track(&video_track, LocalTracksConstraints::default())
        .unwrap();

    let (tx, _rx) = mpsc::unbounded();
    let pc = peer::Component::new(
        peer::PeerConnection::new(
            &pc_state,
            tx,
            Rc::new(MediaManager::default()),
            LocalTracksConstraints::default(),
            Rc::new(Connections::default()),
            Rc::new(RecvConstraints::default()),
        )
        .unwrap(),
        Rc::new(pc_state),
    );
    pc.state().when_all_tracks_created().await;
    pc.state().when_updated().await;

    let audio_track = pc.obj().get_sender_by_id(audio_track_id).unwrap();
    let video_track = pc.obj().get_sender_by_id(video_track_id).unwrap();
    let audio_track_state =
        pc.obj().get_sender_state_by_id(audio_track_id).unwrap();
    let video_track_state =
        pc.obj().get_sender_state_by_id(video_track_id).unwrap();

    assert!(!audio_track.general_disabled());
    assert!(!video_track.general_disabled());

    audio_track_state
        .media_state_transition_to(Disabled.into())
        .unwrap();
    pc.state().patch_track(&TrackPatchEvent {
        id: audio_track_id,
        enabled_general: Some(false),
        enabled_individual: Some(false),
        muted: None,
    });
    pc.state().when_updated().await;
    assert!(audio_track.general_disabled());
    assert!(!video_track.general_disabled());

    video_track_state
        .media_state_transition_to(Disabled.into())
        .unwrap();
    pc.state().patch_track(&TrackPatchEvent {
        id: video_track_id,
        enabled_general: Some(false),
        enabled_individual: Some(false),
        muted: None,
    });
    pc.state().when_updated().await;
    assert!(audio_track.general_disabled());
    assert!(video_track.general_disabled());

    audio_track_state
        .media_state_transition_to(Enabled.into())
        .unwrap();
    pc.state().patch_track(&TrackPatchEvent {
        id: audio_track_id,
        enabled_individual: Some(true),
        enabled_general: Some(true),
        muted: None,
    });
    pc.state().when_updated().await;
    assert!(!audio_track.general_disabled());
    assert!(video_track.general_disabled());

    video_track_state
        .media_state_transition_to(Enabled.into())
        .unwrap();
    pc.state().patch_track(&TrackPatchEvent {
        id: video_track_id,
        enabled_individual: Some(true),
        enabled_general: Some(true),
        muted: None,
    });
    pc.state().when_updated().await;
    assert!(!audio_track.general_disabled());
    assert!(!video_track.general_disabled());
}
