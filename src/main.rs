//! Medea media server application.

use actix::prelude::*;
use dotenv::dotenv;
use hashbrown::HashMap;

use crate::media::{AudioSettings, Track, TrackMediaType, VideoSettings};
use crate::{
    api::{
        client::{server, Room, RoomsRepository},
        control::{Id as MemberId, Member},
    },
    media::peer::{Peer, PeerMachine},
};
use std::sync::Arc;

#[macro_use]
mod utils;

mod api;
mod log;
mod media;

fn main() {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();

    let sys = System::new("medea");

    let members = hashmap! {
        1 => Member{id: 1, credentials: "caller_credentials".to_owned()},
        2 => Member{id: 2, credentials: "responder_credentials".to_owned()},
    };
    let peers = create_peers(1, 2);
    let room = Arbiter::start(move |_| Room::new(1, members, peers));
    let rooms = hashmap! {1 => room};
    let rooms_repo = RoomsRepository::new(rooms);

    server::run(rooms_repo);
    let _ = sys.run();
}

fn create_peers(
    caller: MemberId,
    callee: MemberId,
) -> HashMap<MemberId, PeerMachine> {
    let caller_peer_id = 1;
    let callee_peer_id = 2;
    let mut caller_peer = Peer::new(caller_peer_id, caller, callee_peer_id);
    let mut callee_peer = Peer::new(callee_peer_id, callee, caller_peer_id);

    let track_audio =
        Arc::new(Track::new(1, TrackMediaType::Audio(AudioSettings {})));
    let track_video =
        Arc::new(Track::new(2, TrackMediaType::Video(VideoSettings {})));
    caller_peer.add_sender(track_audio.clone());
    caller_peer.add_sender(track_video.clone());
    callee_peer.add_receiver(track_audio);
    callee_peer.add_receiver(track_video);

    hashmap!(
        caller_peer_id => PeerMachine::New(caller_peer),
        callee_peer_id => PeerMachine::New(callee_peer),
    )
}
