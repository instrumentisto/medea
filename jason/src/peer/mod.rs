//! Adapters to [RTCPeerConnection][1] and related objects.
//!
//! [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface

mod conn;
mod ice_server;
mod media;
mod repo;

use std::{collections::HashMap, rc::Rc};

use futures::channel::mpsc;
use medea_client_api_proto::{
    Direction, IceServer, PeerId as Id, Track, TrackId,
};
use medea_macro::dispatchable;
use web_sys::RtcTrackEvent;

use crate::{
    media::{MediaManager, MediaStream},
    utils::WasmErr,
};

#[cfg(feature = "mockable")]
#[doc(inline)]
pub use self::repo::MockPeerRepository;
#[doc(inline)]
pub use self::repo::{PeerRepository, Repository};
pub use self::{
    conn::{
        IceCandidate, RtcPeerConnection, SdpType, TransceiverDirection,
        TransceiverKind,
    },
    media::MediaConnections,
};

#[dispatchable]
#[allow(clippy::module_name_repetitions)]
/// Events emitted from [`RtcPeerConnection`].
pub enum PeerEvent {
    /// [`RtcPeerConnection`] discovered new ice candidate.
    IceCandidateDiscovered {
        peer_id: Id,
        candidate: String,
        sdp_m_line_index: Option<u16>,
        sdp_mid: Option<String>,
    },

    /// [`RtcPeerConnection`] received new stream from remote sender.
    NewRemoteStream {
        peer_id: Id,
        sender_id: Id,
        remote_stream: MediaStream,
    },
}

struct InnerPeerConnection {
    /// Unique ID of [`PeerConnection`].
    id: Id,

    /// Underlying [`RtcPeerConnection`].
    peer: Rc<RtcPeerConnection>,

    /// [`Sender`]s and [`Receivers`] of this [`RtcPeerConnection`].
    media_connections: MediaConnections,

    /// [`MediaManager`] that will be used to acquire local [`MediaStream`]s.
    media_manager: Rc<MediaManager>,

    /// [`PeerEvent`]s tx.
    peer_events_sender: mpsc::UnboundedSender<PeerEvent>,

    /// Indicates if underlying [`RtcPeerConnection`] has remote description.
    has_remote_description: bool,

    /// Stores [`IceCandidate`]s received before remote description for
    /// underlying [`RtcPeerConnection`].
    ice_candidates_buffer: Vec<IceCandidate>,
}

impl InnerPeerConnection {
    fn new<I: IntoIterator<Item = IceServer>>(
        id: Id,
        ice_servers: I,
        media_manager: Rc<MediaManager>,
        peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
        enabled_audio: bool,
        enabled_video: bool,
    ) -> Result<Self, WasmErr> {
        let peer = Rc::new(RtcPeerConnection::new(ice_servers)?);
        let media_connections = MediaConnections::new(
            Rc::clone(&peer),
            enabled_audio,
            enabled_video,
        );
        Ok(Self {
            id,
            peer,
            media_connections,
            media_manager,
            peer_events_sender,
            has_remote_description: false,
            ice_candidates_buffer: vec![],
        })
    }
}

#[allow(clippy::module_name_repetitions)]
pub struct PeerConnection(Rc<InnerPeerConnection>);

impl PeerConnection {
    /// Creates new [`PeerConnection`].
    ///
    /// Provided `peer_events_sender` will be used to emit [`PeerEvent`]s from
    /// this peer.
    ///
    /// Provided `ice_servers` will be used by created [`RtcPeerConnection`].
    pub fn new<I: IntoIterator<Item = IceServer>>(
        id: Id,
        peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
        ice_servers: I,
        media_manager: Rc<MediaManager>,
        enabled_audio: bool,
        enabled_video: bool,
    ) -> Result<Self, WasmErr> {
        let inner = Rc::new(InnerPeerConnection::new(
            id,
            ice_servers,
            media_manager,
            peer_events_sender,
            enabled_audio,
            enabled_video,
        )?);

        // Bind to `icecandidate` event.
        let inner_rc = Rc::clone(&inner);
        inner.peer.on_ice_candidate(Some(move |candidate| {
            Self::on_ice_candidate(&inner_rc, candidate);
        }))?;

        // Bind to `track` event.
        let inner_rc = Rc::clone(&inner);
        inner.peer.on_track(Some(move |track_event| {
            Self::on_track(&inner_rc, &track_event);
        }))?;

        Ok(Self(inner))
    }

    /// Returns inner [`IceCandidate`]'s buffer len. Used in tests.
    pub fn candidates_buffer_len(&self) -> usize {
        self.0.ice_candidates_buffer.len()
    }

    /// Handle `icecandidate` event from underlying peer emitting
    /// [`PeerEvent::IceCandidateDiscovered`] event into this peers
    /// `peer_events_sender`.
    fn on_ice_candidate(inner: &InnerPeerConnection, candidate: IceCandidate) {
        let _ = inner.peer_events_sender.unbounded_send(
            PeerEvent::IceCandidateDiscovered {
                peer_id: inner.id,
                candidate: candidate.candidate,
                sdp_m_line_index: candidate.sdp_m_line_index,
                sdp_mid: candidate.sdp_mid,
            },
        );
    }

    /// Handle `track` event from underlying peer adding new track to
    /// `media_connections` and emitting [`PeerEvent::NewRemoteStream`]
    /// event into this peers `peer_events_sender` if all tracks from this
    /// sender has arrived.
    fn on_track(inner: &InnerPeerConnection, track_event: &RtcTrackEvent) {
        let transceiver = track_event.transceiver();
        let track = track_event.track();

        if let Some(sender_id) =
            inner.media_connections.add_remote_track(transceiver, track)
        {
            if let Some(tracks) =
                inner.media_connections.get_tracks_by_sender(sender_id)
            {
                // got all tracks from this sender, so emit
                // PeerEvent::NewRemoteStream
                let _ = inner.peer_events_sender.unbounded_send(
                    PeerEvent::NewRemoteStream {
                        peer_id: inner.id,
                        sender_id,
                        remote_stream: MediaStream::from_tracks(tracks),
                    },
                );
            };
        } else {
            // TODO: means that this peer is out of sync, should be
            //       handled somehow (propagated to medea to init peer
            //       recreation?)
        }
    }

    /// Disables or enables all audio tracks for all [`Sender`]s.
    pub fn toggle_send_audio(&self, enabled: bool) {
        self.0
            .media_connections
            .toggle_send_media(TransceiverKind::Audio, enabled)
    }

    /// Disables or enables all video tracks for all [`Sender`]s.
    pub fn toggle_send_video(&self, enabled: bool) {
        self.0
            .media_connections
            .toggle_send_media(TransceiverKind::Video, enabled)
    }

    /// Returns `true` if all [`Sender`]s audio tracks are enabled.
    pub fn is_send_audio_enabled(&self) -> bool {
        self.0
            .media_connections
            .are_senders_enabled(TransceiverKind::Audio)
    }

    /// Returns `true` if all [`Sender`]s video tracks are enabled.
    pub fn is_send_video_enabled(&self) -> bool {
        self.0
            .media_connections
            .are_senders_enabled(TransceiverKind::Video)
    }

    /// Track id to mid relations of all send tracks of this
    /// [`RtcPeerConnection`]. mid is id of [`m= section`][1]. mids are received
    /// directly from registered [`RTCRtpTransceiver`][2]s, and are being
    /// allocated on sdp update.
    /// Errors if finds transceiver without mid, so must be called after setting
    /// local description if offerrer, and remote if answerer.
    ///
    /// [1]: https://tools.ietf.org/html/rfc4566#section-5.14
    /// [2]: https://www.w3.org/TR/webrtc/#rtcrtptransceiver-interface
    pub fn get_mids(&self) -> Result<HashMap<TrackId, String>, WasmErr> {
        self.0.media_connections.get_mids()
    }

    /// Sync provided tracks creating all required `Sender`s and
    /// `Receiver`s, request local stream if required, get, set and return
    /// sdp offer.
    pub async fn get_offer(
        &self,
        tracks: Vec<Track>,
    ) -> Result<String, WasmErr> {
        let request = self.0.media_connections.update_tracks(tracks)?;

        if let Some(request) = request {
            let local_stream = self.0.media_manager.get_stream(request).await?;
            self.0
                .media_connections
                .insert_local_stream(&local_stream)
                .await?;
        }

        self.0.peer.create_and_set_offer().await
    }

    /// Creates an SDP answer to an offer received from a remote peer and sets
    /// it as local description. Must be called only if peer already has remote
    /// description.
    pub async fn create_and_set_answer(&self) -> Result<String, WasmErr> {
        self.0.peer.create_and_set_answer().await
    }

    /// Updates underlying [RTCPeerConnection][1]'s remote SDP from answer.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    pub async fn set_remote_answer(
        &self,
        answer: String,
    ) -> Result<(), WasmErr> {
        self.set_remote_description(SdpType::Answer(answer)).await
    }

    /// Updates underlying [RTCPeerConnection][1]'s remote SDP from offer.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    async fn set_remote_offer(&self, offer: String) -> Result<(), WasmErr> {
        self.set_remote_description(SdpType::Offer(offer)).await
    }

    /// Updates underlying [RTCPeerConnection][1]'s remote SDP with given
    /// description.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    async fn set_remote_description(
        &self,
        desc: SdpType,
    ) -> Result<(), WasmErr> {
        self.0.peer.set_remote_description(desc).await?;
        self.0.has_remote_description = true;

        let candidates = std::mem::replace(
            self.0.ice_candidates_buffer.as_mut(),
            Vec::new(),
        );
        let peer = &self.0.peer;
        for candidate in candidates {
            peer.add_ice_candidate(
                &candidate.candidate,
                candidate.sdp_m_line_index,
                &candidate.sdp_mid,
            )
            .await?;
        }

        Ok(())
    }

    /// Sync provided tracks creating all required `Sender`s and
    /// `Receiver`s, request local stream if required.
    /// `set_remote_description` will create all transceivers and fire all
    /// `on_track` events, so it updates `Receiver`s before
    /// `set_remote_description` and update `Sender`s after.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    pub async fn process_offer(
        &self,
        offer: String,
        tracks: Vec<Track>,
    ) -> Result<(), WasmErr> {
        // TODO: use drain_filter when its stable
        let (recv, send): (Vec<_>, Vec<_>) =
            tracks.into_iter().partition(|track| match track.direction {
                Direction::Send { .. } => false,
                Direction::Recv { .. } => true,
            });

        // update receivers
        self.0.media_connections.update_tracks(recv)?;

        self.set_remote_offer(offer).await?;

        let request = self.0.media_connections.update_tracks(send)?;

        if let Some(request) = request {
            let local_stream = self.0.media_manager.get_stream(request).await?;
            self.0
                .media_connections
                .insert_local_stream(&local_stream)
                .await?;
        }

        Ok(())
    }

    /// Adds remote peers [ICE Candidate][1] to this peer.
    ///
    /// [1]: https://tools.ietf.org/html/rfc5245#section-2
    pub async fn add_ice_candidate(
        &self,
        candidate: String,
        sdp_m_line_index: Option<u16>,
        sdp_mid: Option<String>,
    ) -> Result<(), WasmErr> {
        if self.0.has_remote_description {
            self.0
                .peer
                .add_ice_candidate(&candidate, sdp_m_line_index, &sdp_mid)
                .await?;
        } else {
            self.0.ice_candidates_buffer.push(IceCandidate {
                candidate,
                sdp_m_line_index,
                sdp_mid,
            });
        }

        Ok(())
    }
}

impl Drop for PeerConnection {
    /// Drops `on_track` and `on_ice_candidate` callbacks to prevent leak.
    fn drop(&mut self) {
        let _ = self.0.peer.on_track::<Box<dyn FnMut(RtcTrackEvent)>>(None);
        let _ = self
            .0
            .peer
            .on_ice_candidate::<Box<dyn FnMut(IceCandidate)>>(None);
    }
}
