//! Adapters to [RTCPeerConnection][1] and related objects.
//!
//! [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface

mod conn;
mod ice_server;
mod media;
mod repo;
mod stream;
mod stream_request;
mod track;

use std::{cell::RefCell, collections::HashMap, convert::TryFrom, rc::Rc};

use anyhow::Result;
use futures::{channel::mpsc, future};
use medea_client_api_proto::{
    Direction, IceServer, PeerId as Id, Track, TrackId,
};
use medea_macro::dispatchable;
use web_sys::{MediaStream as SysMediaStream, RtcTrackEvent};

use crate::media::MediaManager;

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
    stream::{MediaStream, MediaStreamHandle},
    stream_request::{Error, SimpleStreamRequest, StreamRequest},
    track::MediaTrack,
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

    /// [`RtcPeerConnection`] send new local stream to remote members.
    NewLocalStream {
        peer_id: Id,
        local_stream: MediaStream,
    },
}

#[allow(clippy::module_name_repetitions)]
pub struct PeerConnection {
    /// Unique ID of [`PeerConnection`].
    id: Id,

    /// Underlying [`RtcPeerConnection`].
    peer: Rc<RtcPeerConnection>,

    /// [`Sender`]s and [`Receivers`] of this [`RtcPeerConnection`].
    media_connections: Rc<MediaConnections>,

    /// [`MediaManager`] that will be used to acquire local [`MediaStream`]s.
    media_manager: Rc<MediaManager>,

    /// [`PeerEvent`]s tx.
    peer_events_sender: mpsc::UnboundedSender<PeerEvent>,

    /// Indicates if underlying [`RtcPeerConnection`] has remote description.
    has_remote_description: RefCell<bool>,

    /// Stores [`IceCandidate`]s received before remote description for
    /// underlying [`RtcPeerConnection`].
    ice_candidates_buffer: RefCell<Vec<IceCandidate>>,
}

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
    ) -> Result<Self> {
        let peer = Rc::new(RtcPeerConnection::new(ice_servers)?);
        let media_connections = Rc::new(MediaConnections::new(
            Rc::clone(&peer),
            enabled_audio,
            enabled_video,
        ));

        let peer = Self {
            id,
            peer,
            media_connections,
            media_manager,
            peer_events_sender,
            has_remote_description: RefCell::new(false),
            ice_candidates_buffer: RefCell::new(vec![]),
        };

        // Bind to `icecandidate` event.
        let id = peer.id;
        let sender = peer.peer_events_sender.clone();
        peer.peer.on_ice_candidate(Some(move |candidate| {
            Self::on_ice_candidate(id, &sender, candidate);
        }))?;

        // Bind to `track` event.
        let id = peer.id;
        let media_connections = Rc::clone(&peer.media_connections);
        let sender = peer.peer_events_sender.clone();
        peer.peer.on_track(Some(move |track_event| {
            Self::on_track(id, &media_connections, &sender, &track_event);
        }))?;

        Ok(peer)
    }

    /// Returns inner [`IceCandidate`]'s buffer len. Used in tests.
    pub fn candidates_buffer_len(&self) -> usize {
        self.ice_candidates_buffer.borrow().len()
    }

    /// Handle `icecandidate` event from underlying peer emitting
    /// [`PeerEvent::IceCandidateDiscovered`] event into this peers
    /// `peer_events_sender`.
    fn on_ice_candidate(
        id: Id,
        sender: &mpsc::UnboundedSender<PeerEvent>,
        candidate: IceCandidate,
    ) {
        let _ = sender.unbounded_send(PeerEvent::IceCandidateDiscovered {
            peer_id: id,
            candidate: candidate.candidate,
            sdp_m_line_index: candidate.sdp_m_line_index,
            sdp_mid: candidate.sdp_mid,
        });
    }

    /// Handle `track` event from underlying peer adding new track to
    /// `media_connections` and emitting [`PeerEvent::NewRemoteStream`]
    /// event into this peers `peer_events_sender` if all tracks from this
    /// sender has arrived.
    fn on_track(
        id: Id,
        media_connections: &MediaConnections,
        sender: &mpsc::UnboundedSender<PeerEvent>,
        track_event: &RtcTrackEvent,
    ) {
        let transceiver = track_event.transceiver();
        let track = track_event.track();

        if let Some(sender_id) =
            media_connections.add_remote_track(transceiver, track)
        {
            if let Some(tracks) =
                media_connections.get_tracks_by_sender(sender_id)
            {
                // got all tracks from this sender, so emit
                // PeerEvent::NewRemoteStream
                let _ = sender.unbounded_send(PeerEvent::NewRemoteStream {
                    peer_id: id,
                    sender_id,
                    remote_stream: MediaStream::from_tracks(tracks),
                });
            };
        } else {
            // TODO: means that this peer is out of sync, should be
            //       handled somehow (propagated to medea to init peer
            //       recreation?)
        }
    }

    /// Disables or enables all audio tracks for all [`Sender`]s.
    pub fn toggle_send_audio(&self, enabled: bool) {
        self.media_connections
            .toggle_send_media(TransceiverKind::Audio, enabled)
    }

    /// Disables or enables all video tracks for all [`Sender`]s.
    pub fn toggle_send_video(&self, enabled: bool) {
        self.media_connections
            .toggle_send_media(TransceiverKind::Video, enabled)
    }

    /// Returns `true` if all [`Sender`]s audio tracks are enabled.
    pub fn is_send_audio_enabled(&self) -> bool {
        self.media_connections
            .are_senders_enabled(TransceiverKind::Audio)
    }

    /// Returns `true` if all [`Sender`]s video tracks are enabled.
    pub fn is_send_video_enabled(&self) -> bool {
        self.media_connections
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
    pub fn get_mids(&self) -> Result<HashMap<TrackId, String>> {
        let mids = self.media_connections.get_mids()?;

        Ok(mids)
    }

    /// Sync provided tracks creating all required `Sender`s and
    /// `Receiver`s, request local stream if required, get, set and return
    /// sdp offer.
    pub async fn get_offer(
        &self,
        tracks: Vec<Track>,
        local_stream: Option<&SysMediaStream>,
    ) -> Result<String> {
        self.media_connections.update_tracks(tracks)?;

        self.insert_local_stream(local_stream).await?;

        let offer = self.peer.create_and_set_offer().await?;

        Ok(offer)
    }

    /// Replace local stream into underlying [RTCPeerConnection][1] on provided
    /// [MediaStream][2] if its have all required tracks.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    pub async fn inject_local_stream(
        &self,
        local_stream: &SysMediaStream,
    ) -> Result<()> {
        self.insert_local_stream(Some(local_stream)).await
    }

    /// Insert provided [MediaStream][1] into underlying [RTCPeerConnection][2]
    /// if it has all required tracks.
    /// Requests local stream from [`MediaManager`] if no stream was provided.
    /// Will produce [`PeerEvent::NewLocalStream`] if new stream was received
    /// from [`MediaManager`].
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    /// [2]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    async fn insert_local_stream(
        &self,
        local_stream: Option<&SysMediaStream>,
    ) -> Result<()> {
        if let Some(request) = self.media_connections.get_stream_request() {
            let caps = SimpleStreamRequest::try_from(request)?;
            let (stream, is_new_stream) = if let Some(stream) = local_stream {
                (caps.parse_stream(stream)?, false)
            } else {
                let (stream, is_new) =
                    self.media_manager.get_stream(&caps).await?;
                (caps.parse_stream(&stream)?, is_new)
            };
            self.media_connections.insert_local_stream(&stream).await?;
            if is_new_stream {
                let _ = self.peer_events_sender.unbounded_send(
                    PeerEvent::NewLocalStream {
                        peer_id: self.id,
                        local_stream: stream,
                    },
                );
            }
        }
        Ok(())
    }

    /// Updates underlying [RTCPeerConnection][1]'s remote SDP from answer.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    pub async fn set_remote_answer(&self, answer: String) -> Result<()> {
        self.set_remote_description(SdpType::Answer(answer)).await
    }

    /// Updates underlying [RTCPeerConnection][1]'s remote SDP from offer.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    async fn set_remote_offer(&self, offer: String) -> Result<()> {
        self.set_remote_description(SdpType::Offer(offer)).await
    }

    /// Updates underlying [RTCPeerConnection][1]'s remote SDP with given
    /// description.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    async fn set_remote_description(&self, desc: SdpType) -> Result<()> {
        self.peer.set_remote_description(desc).await?;
        *self.has_remote_description.borrow_mut() = true;

        let mut futures = Vec::new();
        let mut candidates = self.ice_candidates_buffer.borrow_mut();
        while let Some(candidate) = candidates.pop() {
            let peer = Rc::clone(&self.peer);
            futures.push(async move {
                peer.add_ice_candidate(
                    &candidate.candidate,
                    candidate.sdp_m_line_index,
                    &candidate.sdp_mid,
                )
                .await
            });
        }
        for res in future::join_all(futures).await {
            res?;
        }
        Ok(())
    }

    /// Sync provided tracks creating all required `Sender`s and
    /// `Receiver`s, request local stream if required, get, set and return
    /// sdp answer.
    /// `set_remote_description` will create all transceivers and fire all
    /// `on_track` events, so it updates `Receiver`s before
    /// `set_remote_description` and update `Sender`s after.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    pub async fn process_offer(
        &self,
        offer: String,
        tracks: Vec<Track>,
        local_stream: Option<&SysMediaStream>,
    ) -> Result<String> {
        // TODO: use drain_filter when its stable
        let (recv, send): (Vec<_>, Vec<_>) =
            tracks.into_iter().partition(|track| match track.direction {
                Direction::Send { .. } => false,
                Direction::Recv { .. } => true,
            });

        // update receivers
        self.media_connections.update_tracks(recv)?;

        self.set_remote_offer(offer).await?;

        self.media_connections.update_tracks(send)?;

        self.insert_local_stream(local_stream).await?;

        let answer = self.peer.create_and_set_answer().await?;

        Ok(answer)
    }

    /// Adds remote peers [ICE Candidate][1] to this peer.
    ///
    /// [1]: https://tools.ietf.org/html/rfc5245#section-2
    pub async fn add_ice_candidate(
        &self,
        candidate: String,
        sdp_m_line_index: Option<u16>,
        sdp_mid: Option<String>,
    ) -> Result<()> {
        if *self.has_remote_description.borrow() {
            self.peer
                .add_ice_candidate(&candidate, sdp_m_line_index, &sdp_mid)
                .await?;
        } else {
            self.ice_candidates_buffer.borrow_mut().push(IceCandidate {
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
        let _ = self.peer.on_track::<Box<dyn FnMut(RtcTrackEvent)>>(None);
        let _ = self
            .peer
            .on_ice_candidate::<Box<dyn FnMut(IceCandidate)>>(None);
    }
}
