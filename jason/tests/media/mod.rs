mod constraints;
mod manager;
mod track;

use std::{convert::TryFrom, rc::Rc};

use futures::channel::mpsc;
use medea_client_api_proto::{
    AudioSettings, Direction, MediaType, MemberId, Track, TrackId,
};
use medea_jason::{
    media::{MediaManager, RecvConstraints},
    peer::{LocalStreamUpdateCriteria, MediaConnections, SimpleTracksRequest},
    platform::{RtcPeerConnection, TransceiverDirection},
};
use wasm_bindgen_test::*;

use crate::get_media_stream_settings;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn sendrecv_works() {
    let (tx, _rx) = mpsc::unbounded();
    let media_connections = MediaConnections::new(
        Rc::new(RtcPeerConnection::new(Vec::new(), false).unwrap()),
        tx,
    );
    let send_audio_track = Track {
        id: TrackId(1),
        direction: Direction::Send {
            receivers: vec![MemberId::from("bob")],
            mid: None,
        },
        media_type: MediaType::Audio(AudioSettings { required: false }),
    };
    let recv_audio_track = Track {
        id: TrackId(2),
        direction: Direction::Recv {
            mid: None,
            sender: MemberId::from("alice"),
        },
        media_type: MediaType::Audio(AudioSettings { required: false }),
    };
    media_connections
        .create_tracks(
            vec![send_audio_track, recv_audio_track],
            &get_media_stream_settings(true, false).into(),
            &RecvConstraints::default(),
        )
        .unwrap();
    let request = media_connections
        .get_tracks_request(LocalStreamUpdateCriteria::all())
        .unwrap();
    let caps = SimpleTracksRequest::try_from(request).unwrap();
    let manager = Rc::new(MediaManager::default());
    let tracks = manager.get_tracks(&caps).await.unwrap();

    media_connections
        .insert_local_tracks(
            &caps
                .parse_tracks(tracks.into_iter().map(|(t, _)| t).collect())
                .unwrap(),
        )
        .await
        .unwrap();
    let video_sender = media_connections.get_sender_by_id(TrackId(1)).unwrap();
    let video_receiver =
        media_connections.get_receiver_by_id(TrackId(2)).unwrap();

    assert!(video_sender.is_publishing());
    assert!(video_receiver.is_receiving());

    assert!(video_sender.transceiver().has_direction(
        TransceiverDirection::SEND | TransceiverDirection::RECV
    ));
    assert!(video_receiver.transceiver().unwrap().has_direction(
        TransceiverDirection::SEND | TransceiverDirection::RECV
    ));
}
