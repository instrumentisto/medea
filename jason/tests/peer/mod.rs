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
        RtcStatsType, StatId, TrackStat, TrackStatKind,
    },
    AudioSettings, Direction, IceConnectionState, MediaType, PeerId, Track,
    TrackId, TrackPatch, VideoSettings,
};
use medea_jason::{
    media::MediaManager,
    peer::{
        PeerConnection, PeerEvent, RtcStats, StableMuteState, TransceiverKind,
    },
};
use wasm_bindgen_test::*;

use crate::{
    delay_for, get_media_stream_settings, get_test_unrequired_tracks, timeout,
};
use medea_jason::media::LocalStreamConstraints;

wasm_bindgen_test_configure!(run_in_browser);

fn toggle_mute_tracks_updates(
    tracks_ids: &[u32],
    is_muted: bool,
) -> Vec<TrackPatch> {
    tracks_ids
        .into_iter()
        .map(|track_id| TrackPatch {
            id: TrackId(*track_id),
            is_muted: Some(is_muted),
        })
        .collect()
}

const AUDIO_TRACK_ID: u32 = 1;
const VIDEO_TRACK_ID: u32 = 2;

#[wasm_bindgen_test]
async fn mute_unmute_audio() {
    let (tx, _rx) = mpsc::unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_unrequired_tracks();
    let peer = PeerConnection::new(
        PeerId(1),
        tx,
        Vec::new(),
        manager,
        false,
        LocalStreamConstraints::default(),
    )
    .unwrap();

    peer.get_offer(vec![audio_track, video_track])
        .await
        .unwrap();

    assert!(peer.is_send_audio_enabled());
    assert!(peer.is_send_video_enabled());

    peer.update_senders(toggle_mute_tracks_updates(&[AUDIO_TRACK_ID], true))
        .unwrap();
    assert!(!peer.is_send_audio_enabled());
    assert!(peer.is_send_video_enabled());

    peer.update_senders(toggle_mute_tracks_updates(&[AUDIO_TRACK_ID], false))
        .unwrap();
    assert!(peer.is_send_audio_enabled());
    assert!(peer.is_send_video_enabled());
}

#[wasm_bindgen_test]
async fn mute_unmute_video() {
    let (tx, _rx) = mpsc::unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_unrequired_tracks();
    let peer = PeerConnection::new(
        PeerId(1),
        tx,
        Vec::new(),
        manager,
        false,
        LocalStreamConstraints::default(),
    )
    .unwrap();
    peer.get_offer(vec![audio_track, video_track])
        .await
        .unwrap();

    assert!(peer.is_send_audio_enabled());
    assert!(peer.is_send_video_enabled());

    peer.update_senders(toggle_mute_tracks_updates(&[VIDEO_TRACK_ID], true))
        .unwrap();
    assert!(peer.is_send_audio_enabled());
    assert!(!peer.is_send_video_enabled());

    peer.update_senders(toggle_mute_tracks_updates(&[VIDEO_TRACK_ID], false))
        .unwrap();
    assert!(peer.is_send_audio_enabled());
    assert!(peer.is_send_video_enabled());
}

#[wasm_bindgen_test]
async fn new_with_mute_audio() {
    let (tx, _rx) = mpsc::unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_unrequired_tracks();
    let peer = PeerConnection::new(
        PeerId(1),
        tx,
        Vec::new(),
        manager,
        false,
        get_media_stream_settings(true, false).into(),
    )
    .unwrap();

    peer.get_offer(vec![audio_track, video_track])
        .await
        .unwrap();
    assert!(!peer.is_send_audio_enabled());

    assert!(peer.is_send_video_enabled());
}

#[wasm_bindgen_test]
async fn new_with_mute_video() {
    let (tx, _rx) = mpsc::unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_unrequired_tracks();
    let peer = PeerConnection::new(
        PeerId(1),
        tx,
        Vec::new(),
        manager,
        false,
        get_media_stream_settings(false, true).into(),
    )
    .unwrap();
    peer.get_offer(vec![audio_track, video_track])
        .await
        .unwrap();

    assert!(peer.is_send_audio_enabled());
    assert!(!peer.is_send_video_enabled());
}

#[wasm_bindgen_test]
async fn add_candidates_to_answerer_before_offer() {
    let (tx1, rx1) = mpsc::unbounded();
    let (tx2, _) = mpsc::unbounded();

    let manager = Rc::new(MediaManager::default());
    let pc1 = PeerConnection::new(
        PeerId(1),
        tx1,
        Vec::new(),
        Rc::clone(&manager),
        false,
        LocalStreamConstraints::default(),
    )
    .unwrap();

    let pc2 = PeerConnection::new(
        PeerId(2),
        tx2,
        Vec::new(),
        manager,
        false,
        LocalStreamConstraints::default(),
    )
    .unwrap();
    let (audio_track, video_track) = get_test_unrequired_tracks();
    let offer = pc1.get_offer(vec![audio_track, video_track]).await.unwrap();

    handle_ice_candidates(rx1, &pc2, 1).await;
    // assert that pc2 has buffered candidates
    assert!(pc2.candidates_buffer_len() > 0);
    // then set its remote description
    pc2.process_offer(offer, Vec::new()).await.unwrap();

    // and assert that buffer was flushed
    assert_eq!(pc2.candidates_buffer_len(), 0);
}

#[wasm_bindgen_test]
async fn add_candidates_to_offerer_before_answer() {
    let (tx1, _) = mpsc::unbounded();
    let (tx2, rx2) = mpsc::unbounded();

    let manager = Rc::new(MediaManager::default());
    let pc1 = Rc::new(
        PeerConnection::new(
            PeerId(1),
            tx1,
            Vec::new(),
            Rc::clone(&manager),
            false,
            LocalStreamConstraints::default(),
        )
        .unwrap(),
    );
    let pc2 = Rc::new(
        PeerConnection::new(
            PeerId(2),
            tx2,
            Vec::new(),
            manager,
            false,
            LocalStreamConstraints::default(),
        )
        .unwrap(),
    );

    let (audio_track, video_track) = get_test_unrequired_tracks();
    let offer = pc1.get_offer(vec![audio_track, video_track]).await.unwrap();
    let answer = pc2.process_offer(offer, Vec::new()).await.unwrap();

    handle_ice_candidates(rx2, &pc1, 1).await;

    // assert that pc1 has buffered candidates
    assert!(pc1.candidates_buffer_len() > 0);
    pc1.set_remote_answer(answer).await.unwrap();
    // assert that pc1 has buffered candidates got fulshed
    assert_eq!(pc1.candidates_buffer_len(), 0);
}

#[wasm_bindgen_test]
async fn normal_exchange_of_candidates() {
    let (tx1, rx1) = mpsc::unbounded();
    let (tx2, rx2) = mpsc::unbounded();

    let manager = Rc::new(MediaManager::default());
    let peer1 = PeerConnection::new(
        PeerId(1),
        tx1,
        Vec::new(),
        Rc::clone(&manager),
        false,
        LocalStreamConstraints::default(),
    )
    .unwrap();
    let peer2 = PeerConnection::new(
        PeerId(2),
        tx2,
        Vec::new(),
        manager,
        false,
        LocalStreamConstraints::default(),
    )
    .unwrap();
    let (audio_track, video_track) = get_test_unrequired_tracks();

    let offer = peer1
        .get_offer(vec![audio_track.clone(), video_track.clone()])
        .await
        .unwrap();
    let answer = peer2
        .process_offer(offer, vec![audio_track, video_track])
        .await
        .unwrap();
    peer1.set_remote_answer(answer).await.unwrap();

    delay_for(500).await;

    handle_ice_candidates(rx1, &peer2, 1).await;
    handle_ice_candidates(rx2, &peer1, 1).await;
}

async fn handle_ice_candidates(
    mut candidates_rx: mpsc::UnboundedReceiver<PeerEvent>,
    peer: &PeerConnection,
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
            PeerEvent::NewLocalStream { .. } => {}
            _ => unreachable!(),
        }
    }
}

#[wasm_bindgen_test]
async fn send_event_on_new_local_stream() {
    let (tx, mut rx) = mpsc::unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_unrequired_tracks();
    let id = PeerId(1);
    let peer = PeerConnection::new(
        id,
        tx,
        Vec::new(),
        manager,
        false,
        get_media_stream_settings(false, true).into(),
    )
    .unwrap();
    peer.get_offer(vec![audio_track, video_track])
        .await
        .unwrap();

    while let Some(event) = rx.next().await {
        match event {
            PeerEvent::NewLocalStream { peer_id, .. } => {
                assert_eq!(peer_id, id);
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

    let manager = Rc::new(MediaManager::default());
    let peer1 = PeerConnection::new(
        PeerId(1),
        tx1,
        Vec::new(),
        Rc::clone(&manager),
        false,
        LocalStreamConstraints::default(),
    )
    .unwrap();
    let peer2 = PeerConnection::new(
        PeerId(2),
        tx2,
        Vec::new(),
        manager,
        false,
        LocalStreamConstraints::default(),
    )
    .unwrap();
    let (audio_track, video_track) = get_test_unrequired_tracks();

    let offer = peer1
        .get_offer(vec![audio_track.clone(), video_track.clone()])
        .await
        .unwrap();
    let answer = peer2
        .process_offer(offer, vec![audio_track, video_track])
        .await
        .unwrap();
    peer1.set_remote_answer(answer).await.unwrap();

    delay_for(500).await;

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
                    peer2
                        .add_ice_candidate(candidate, sdp_m_line_index, sdp_mid)
                        .await
                        .unwrap();
                } else {
                    peer1
                        .add_ice_candidate(candidate, sdp_m_line_index, sdp_mid)
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
    pub first_peer: Rc<PeerConnection>,

    /// This [`PeerConnection`] will have one `video` track with `recv`
    /// direction and one `audio` track with `recv` direction.
    pub second_peer: Rc<PeerConnection>,

    /// All [`PeerEvent`]s of this two interconnected [`PeerConnection`]s.
    pub peer_events_recv: Pin<Box<dyn Stream<Item = PeerEvent>>>,
}

impl InterconnectedPeers {
    /// Creates new interconnected [`PeerConnection`]s.
    pub async fn new() -> Self {
        let (tx1, peer_events_stream1) = mpsc::unbounded();
        let (tx2, peer_events_stream2) = mpsc::unbounded();

        let manager = Rc::new(MediaManager::default());
        let peer1 = PeerConnection::new(
            PeerId(1),
            tx1,
            Vec::new(),
            Rc::clone(&manager),
            false,
            LocalStreamConstraints::default(),
        )
        .unwrap();
        let peer2 = PeerConnection::new(
            PeerId(2),
            tx2,
            Vec::new(),
            manager,
            false,
            LocalStreamConstraints::default(),
        )
        .unwrap();

        let offer = peer1.get_offer(Self::get_peer1_tracks()).await.unwrap();
        let answer = peer2
            .process_offer(offer, Self::get_peer2_tracks())
            .await
            .unwrap();
        peer1.set_remote_answer(answer).await.unwrap();

        delay_for(1000).await;

        let events =
            futures::stream::select(peer_events_stream1, peer_events_stream2);

        let mut interconnected_peers = Self {
            first_peer: peer1,
            second_peer: peer2,
            peer_events_recv: Box::pin(events),
        };

        interconnected_peers.handle_ice_candidates().await;

        interconnected_peers
    }

    /// Handles [`PeerEvent::IceCandidateDiscovered`] and
    /// [`PeerEvent::IceConnectionStateChange`] events.
    ///
    /// This [`Future`] will be resolved when all needed ICE candidates will be
    /// received and [`PeerConnection`]'s ICE connection state will transit into
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
                    receivers: vec![PeerId(2)],
                    mid: None,
                },
                media_type: MediaType::Audio(AudioSettings {
                    is_required: true,
                }),
            },
            Track {
                id: TrackId(2),
                direction: Direction::Send {
                    receivers: vec![PeerId(2)],
                    mid: None,
                },
                media_type: MediaType::Video(VideoSettings {
                    is_required: true,
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
                    sender: PeerId(1),
                    mid: None,
                },
                media_type: MediaType::Audio(AudioSettings {
                    is_required: true,
                }),
            },
            Track {
                id: TrackId(2),
                direction: Direction::Recv {
                    sender: PeerId(2),
                    mid: None,
                },
                media_type: MediaType::Video(VideoSettings {
                    is_required: true,
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
                unreachable!("First Peer shouldn't have any InboundRtp stats.")
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
        let peer = PeerConnection::new(
            PeerId(1),
            tx,
            Vec::new(),
            manager,
            false,
            LocalStreamConstraints::default(),
        )
        .unwrap();

        let stat = RtcStat {
            id: StatId("2ef2e34c".to_string()),
            timestamp: HighResTimeStamp(1584373509700.0),
            stats: RtcStatsType::Track(Box::new(TrackStat {
                track_identifier: "0d4f8e05-51d8-4f9b-90b2-453401fc8041"
                    .to_string(),
                kind: Some(TrackStatKind::Audio),
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
        let peer = PeerConnection::new(
            PeerId(1),
            tx,
            Vec::new(),
            manager,
            false,
            LocalStreamConstraints::default(),
        )
        .unwrap();

        let mut stat = RtcStat {
            id: StatId("2ef2e34c".to_string()),
            timestamp: HighResTimeStamp(1584373509700.0),
            stats: RtcStatsType::Track(Box::new(TrackStat {
                track_identifier: "0d4f8e05-51d8-4f9b-90b2-453401fc8041"
                    .to_string(),
                kind: Some(TrackStatKind::Audio),
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
        let peer = PeerConnection::new(
            PeerId(1),
            tx,
            Vec::new(),
            manager,
            false,
            LocalStreamConstraints::default(),
        )
        .unwrap();

        let mut track_stat = Box::new(TrackStat {
            track_identifier: "0d4f8e05-51d8-4f9b-90b2-453401fc8041"
                .to_string(),
            kind: Some(TrackStatKind::Audio),
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
    let (audio_track, video_track) = get_test_unrequired_tracks();
    let peer = PeerConnection::new(
        PeerId(1),
        tx,
        Vec::new(),
        manager,
        false,
        LocalStreamConstraints::default(),
    )
    .unwrap();
    peer.get_offer(vec![audio_track, video_track])
        .await
        .unwrap();

    let all_unmuted = future::join_all(
        peer.get_senders(TransceiverKind::Audio)
            .into_iter()
            .chain(peer.get_senders(TransceiverKind::Video).into_iter())
            .map(|s| {
                s.mute_state_transition_to(StableMuteState::Muted).unwrap();

                s.when_mute_state_stable(StableMuteState::NotMuted)
            }),
    )
    .map(|_| ())
    .shared();

    delay_for(400).await;
    peer.stop_state_transitions_timers();
    timeout(600, all_unmuted.clone()).await.unwrap_err();

    peer.stop_state_transitions_timers();
    delay_for(30).await;
    peer.reset_state_transitions_timers();

    timeout(600, all_unmuted).await.unwrap();
}
