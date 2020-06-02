//! [MediaStream][1] related objects.
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#mediastream

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

use medea_client_api_proto::{PeerId, TrackId};
use wasm_bindgen::{prelude::*, JsValue};
use web_sys::MediaStream as SysMediaStream;

use crate::{
    media::{MediaStreamTrack, TrackKind},
    utils::HandlerDetachedError,
};

/// Actual data of a [`PeerMediaStream`].
///
/// Shared between JS side ([`RemoteMediaStream`]) and Rust side
/// ([`PeerMediaStream`]).
struct InnerStream {
    /// [`PeerId`] of the remote [`Sender`] from which this [`PeerMediaStream`]
    /// receives media.
    ///
    /// If `None` then this is local [`PeerMediaStream`].
    sender_peer_id: Option<PeerId>,

    /// Actual underlying [MediaStream][1] object.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    stream: SysMediaStream,

    /// List of audio tracks.
    audio_tracks: HashMap<TrackId, MediaStreamTrack>,

    /// List of video tracks.
    video_tracks: HashMap<TrackId, MediaStreamTrack>,
}

impl InnerStream {
    /// Instantiates new remote [`InnerStream`].
    fn new(sender_peer_id: PeerId) -> Self {
        Self {
            sender_peer_id: Some(sender_peer_id),
            stream: SysMediaStream::new().unwrap(),
            audio_tracks: HashMap::new(),
            video_tracks: HashMap::new(),
        }
    }

    /// Instantiates new local [`InnerStream`].
    fn new_local() -> Self {
        Self {
            sender_peer_id: None,
            stream: SysMediaStream::new().unwrap(),
            audio_tracks: HashMap::new(),
            video_tracks: HashMap::new(),
        }
    }

    /// Adds provided [`MediaStreamTrack`] to a stream.
    fn add_track(&mut self, track_id: TrackId, track: MediaStreamTrack) {
        self.stream.add_track(track.as_ref());
        match track.kind() {
            TrackKind::Audio => {
                self.audio_tracks.insert(track_id, track);
            }
            TrackKind::Video => {
                self.video_tracks.insert(track_id, track);
            }
        }
    }
}

/// Representation of [MediaStream][1] object. Each of its tracks has
/// association with [`TrackId`].
///
/// It's used on Rust side and represents a handle to [`InnerStream`] data.
///
/// For using [`PeerMediaStream`] on JS side, consider the
/// [`RemoteMediaStream`].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
pub struct PeerMediaStream(Rc<RefCell<InnerStream>>);

#[allow(clippy::new_without_default)]
impl PeerMediaStream {
    /// Creates empty remote [`PeerMediaStream`].
    pub fn new(sender_peer_id: PeerId) -> Self {
        Self(Rc::new(RefCell::new(InnerStream::new(sender_peer_id))))
    }

    /// Creates empty local [`PeerMediaStream`].
    pub fn new_local() -> Self {
        Self(Rc::new(RefCell::new(InnerStream::new_local())))
    }

    /// Adds provided [`MediaStreamTrack`] to a stream.
    pub fn add_track(&self, track_id: TrackId, track: MediaStreamTrack) {
        self.0.borrow_mut().add_track(track_id, track);
    }

    /// Checks if [`PeerMediaStream`] contains a [`MediaStreamTrack`] with
    /// specified ID.
    pub fn has_track(&self, id: TrackId) -> bool {
        let inner = self.0.borrow();
        inner.video_tracks.contains_key(&id)
            || inner.audio_tracks.contains_key(&id)
    }

    /// Returns a [`MediaStreamTrack`] of [`PeerMediaStream`] by its ID, if any.
    pub fn get_track_by_id(
        &self,
        track_id: TrackId,
    ) -> Option<MediaStreamTrack> {
        let inner = self.0.borrow();
        match inner.video_tracks.get(&track_id) {
            Some(track) => Some(track.clone()),
            None => match inner.audio_tracks.get(&track_id) {
                Some(track) => Some(track.clone()),
                None => None,
            },
        }
    }

    /// Instantiates new [`RemoteMediaStream`] for use on JS side.
    pub fn new_handle(&self) -> RemoteMediaStream {
        RemoteMediaStream(Rc::downgrade(&self.0))
    }

    /// Returns actual underlying [MediaStream][1] object.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    pub fn stream(&self) -> SysMediaStream {
        Clone::clone(&self.0.borrow().stream)
    }

    /// Returns `true` if this [`PeerMediaStream`] doesn't have any
    /// [`MediaTrack`]s.
    pub fn is_empty(&self) -> bool {
        let inner = self.0.borrow();
        inner.audio_tracks.is_empty() && inner.video_tracks.is_empty()
    }
}

/// JS side handle to [`PeerMediaStream`].
///
/// Actually, represents a [`Weak`]-based handle to `InnerStream`.
///
/// For using [`RemoteMediaStream`] on Rust side, consider the
/// [`PeerMediaStream`].
// TODO: This structure named remote, but used as local. Maybe rename it????
#[wasm_bindgen]
pub struct RemoteMediaStream(Weak<RefCell<InnerStream>>);

#[wasm_bindgen]
impl RemoteMediaStream {
    /// Returns the underlying [`PeerMediaStream`][`SysMediaStream`] object.
    pub fn get_media_stream(&self) -> Result<SysMediaStream, JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| Clone::clone(&inner.borrow().stream))
    }

    /// Returns [`PeerId`] of the sender [`PeerConnection`].
    ///
    /// Return `null` if this is local [`RemoteMediaStream`].
    pub fn sender_peer_id(&self) -> Result<Option<u64>, JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| inner.borrow().sender_peer_id.map(|id| id.0))
    }
}
