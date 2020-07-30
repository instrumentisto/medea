//! [`crate::peer::PeerConnection`] media management.

mod mute_state;
mod receiver;
mod sender;

use std::{
    borrow::ToOwned, cell::RefCell, collections::HashMap, convert::From, rc::Rc,
};

use derive_more::Display;
use futures::{channel::mpsc, future};
use medea_client_api_proto as proto;
use medea_reactive::DroppedError;
use proto::{Direction, PeerId, Track, TrackId};
use tracerr::Traced;
use web_sys::RtcRtpTransceiver;

use crate::{
    media::{LocalStreamConstraints, MediaStreamTrack},
    peer::PeerEvent,
    utils::{JsCaused, JsError},
};

use super::{
    conn::{RtcPeerConnection, TransceiverKind},
    stream::PeerMediaStream,
    stream_request::StreamRequest,
};

use self::sender::SenderBuilder;

pub use self::{
    mute_state::{MuteState, MuteStateTransition, StableMuteState},
    receiver::Receiver,
    sender::Sender,
};

/// Errors that may occur in [`MediaConnections`] storage.
#[derive(Debug, Display, JsCaused)]
pub enum MediaConnectionsError {
    /// Occurs when the provided [`MediaStreamTrack`] cannot be inserted into
    /// provided [`Sender`]s transceiver.
    #[display(fmt = "Failed to insert Track to a sender: {}", _0)]
    CouldNotInsertLocalTrack(JsError),

    /// Occurs when [`MediaStreamTrack`] discovered by [`RtcPeerConnection`]
    /// could not be inserted into [`Receiver`].
    #[display(
        fmt = "Could not insert remote track with mid: {:?} into media \
               connections",
        _0
    )]
    CouldNotInsertRemoteTrack(Option<String>),

    /// Could not find [`RtcRtpTransceiver`] by `mid`.
    #[display(fmt = "Unable to find Transceiver with provided mid: {}", _0)]
    TransceiverNotFound(String),

    /// Occurs when cannot get the `mid` from the [`Sender`].
    #[display(fmt = "Peer has senders without mid")]
    SendersWithoutMid,

    /// Occurs when cannot get the `mid` from the [`Receiver`].
    #[display(fmt = "Peer has receivers without mid")]
    ReceiversWithoutMid,

    /// Occurs when inserted [`PeerMediaStream`] dont have all necessary
    /// [`MediaStreamTrack`]s.
    #[display(fmt = "Provided stream does not have all necessary Tracks")]
    InvalidMediaStream,

    /// Occurs when [`MediaStreamTrack`] of inserted [`PeerMediaStream`] does
    /// not satisfy [`Sender`] constraints.
    #[display(fmt = "Provided Track does not satisfy senders constraints")]
    InvalidMediaTrack,

    /// Occurs when [`MuteState`] of [`Sender`] was dropped.
    #[display(fmt = "MuteState of Sender was dropped.")]
    MuteStateDropped,

    /// Occurs when [`MuteState`] of [`Sender`] transits into opposite to
    /// expected [`MuteState`].
    #[display(fmt = "MuteState of Sender transits into opposite to expected \
                     MuteState")]
    MuteStateTransitsIntoOppositeState,

    /// Invalid [`medea_client_api_proto::TrackPatch`] for
    /// [`MediaStreamTrack`].
    #[display(fmt = "Invalid TrackPatch for Track with {} ID.", _0)]
    InvalidTrackPatch(TrackId),

    /// Some [`Sender`] can't be muted because it required.
    #[display(fmt = "MuteState of Sender can't be transited into muted \
                     state, because this Sender is required.")]
    CannotDisableRequiredSender,
}

impl From<DroppedError> for MediaConnectionsError {
    #[inline]
    fn from(_: DroppedError) -> Self {
        Self::MuteStateDropped
    }
}

type Result<T> = std::result::Result<T, Traced<MediaConnectionsError>>;

/// Actual data of [`MediaConnections`] storage.
struct InnerMediaConnections {
    /// [`PeerId`] of peer that owns this [`MediaConnections`].
    peer_id: PeerId,

    /// Ref to parent [`RtcPeerConnection`]. Used to generate transceivers for
    /// [`Sender`]s and [`Receiver`]s.
    peer: Rc<RtcPeerConnection>,

    /// [`PeerEvent`]s tx.
    peer_events_sender: mpsc::UnboundedSender<PeerEvent>,

    /// [`TrackId`] to its [`Sender`].
    senders: HashMap<TrackId, Rc<Sender>>,

    /// [`TrackId`] to its [`Receiver`].
    receivers: HashMap<TrackId, Receiver>,
}

impl InnerMediaConnections {
    /// Returns [`Iterator`] over [`Sender`]s with provided [`TransceiverKind`].
    pub fn iter_senders_with_kind(
        &self,
        kind: TransceiverKind,
    ) -> impl Iterator<Item = &Rc<Sender>> {
        self.senders.values().filter(move |s| s.kind() == kind)
    }
}

/// Storage of [`RtcPeerConnection`]'s [`Sender`] and [`Receiver`] tracks.
pub struct MediaConnections(RefCell<InnerMediaConnections>);

impl MediaConnections {
    /// Instantiates new [`MediaConnections`] storage for a given
    /// [`RtcPeerConnection`].
    #[inline]
    pub fn new(
        peer_id: PeerId,
        peer: Rc<RtcPeerConnection>,
        peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
    ) -> Self {
        Self(RefCell::new(InnerMediaConnections {
            peer_id,
            peer,
            peer_events_sender,
            senders: HashMap::new(),
            receivers: HashMap::new(),
        }))
    }

    /// Returns all [`Sender`]s from this [`MediaConnections`] with provided
    /// [`TransceiverKind`].
    pub fn get_senders(&self, kind: TransceiverKind) -> Vec<Rc<Sender>> {
        self.0
            .borrow()
            .iter_senders_with_kind(kind)
            .cloned()
            .collect()
    }

    pub fn get_receivers_ids(&self, kind: TransceiverKind) -> Vec<TrackId> {
        self.0
            .borrow_mut()
            .receivers
            .values()
            .filter(move |s| s.kind() == kind)
            .map(|t| t.track_id())
            .collect()
    }

    /// Returns `true` if all [`Sender`]s with provided [`TransceiverKind`] is
    /// in provided [`MuteState`].
    pub fn is_all_senders_in_mute_state(
        &self,
        kind: TransceiverKind,
        mute_state: StableMuteState,
    ) -> bool {
        for sender in self.0.borrow().iter_senders_with_kind(kind) {
            if sender.mute_state() != mute_state.into() {
                return false;
            }
        }
        true
    }

    /// Returns `true` if all [`Sender`]s with
    /// [`TransceiverKind::Audio`] are enabled or `false` otherwise.
    pub fn is_send_audio_enabled(&self) -> bool {
        self.0
            .borrow()
            .iter_senders_with_kind(TransceiverKind::Audio)
            .find(|s| s.is_muted())
            .is_none()
    }

    /// Returns `true` if all [`Sender`]s with
    /// [`TransceiverKind::Video`] are enabled or `false` otherwise.
    pub fn is_send_video_enabled(&self) -> bool {
        self.0
            .borrow()
            .iter_senders_with_kind(TransceiverKind::Video)
            .find(|s| s.is_muted())
            .is_none()
    }

    /// Returns mapping from a [`MediaStreamTrack`] ID to a `mid` of
    /// this track's [`RtcRtpTransceiver`].
    ///
    /// # Errors
    ///
    /// Errors with [`MediaConnectionsError::SendersWithoutMid`] if some
    /// [`Sender`] doesn't have [mid].
    ///
    /// Errors with [`MediaConnectionsError::ReceiversWithoutMid`] if some
    /// [`Receiver`] doesn't have [mid].
    ///
    /// [mid]:
    /// https://developer.mozilla.org/en-US/docs/Web/API/RTCRtpTransceiver/mid
    pub fn get_mids(&self) -> Result<HashMap<TrackId, String>> {
        let mut inner = self.0.borrow_mut();
        let mut mids =
            HashMap::with_capacity(inner.senders.len() + inner.receivers.len());
        for (track_id, sender) in &inner.senders {
            mids.insert(
                *track_id,
                sender
                    .transceiver
                    .mid()
                    .ok_or(MediaConnectionsError::SendersWithoutMid)
                    .map_err(tracerr::wrap!())?,
            );
        }
        for (track_id, receiver) in &mut inner.receivers {
            mids.insert(
                *track_id,
                receiver
                    .mid()
                    .map(ToOwned::to_owned)
                    .ok_or(MediaConnectionsError::ReceiversWithoutMid)
                    .map_err(tracerr::wrap!())?,
            );
        }
        Ok(mids)
    }

    /// Returns publishing statuses of the all [`Sender`]s from this
    /// [`MediaConnections`].
    pub fn get_senders_statuses(&self) -> HashMap<TrackId, bool> {
        let inner = self.0.borrow();

        let mut out = HashMap::new();
        for (track_id, sender) in &inner.senders {
            out.insert(*track_id, sender.is_publishing());
        }
        out
    }

    /// Creates new [`Sender`]s and [`Receiver`]s for each new [`Track`].
    ///
    /// # Errors
    ///
    /// With [`MediaConnectionsError::TransceiverNotFound`] if could not create
    /// new [`Sender`] cause transceiver with specified `mid` does not
    /// exist.
    pub fn create_tracks<I: IntoIterator<Item = Track>>(
        &self,
        tracks: I,
        local_constraints: &LocalStreamConstraints,
    ) -> Result<()> {
        let mut inner = self.0.borrow_mut();
        for track in tracks {
            let is_required = track.is_required();
            match track.direction {
                Direction::Send { mid, .. } => {
                    let mute_state;
                    if local_constraints.is_enabled(&track.media_type) {
                        mute_state = StableMuteState::NotMuted;
                    } else if is_required {
                        use MediaConnectionsError as Error;
                        return Err(tracerr::new!(
                            Error::CannotDisableRequiredSender
                        ));
                    } else {
                        mute_state = StableMuteState::Muted;
                    }
                    let sndr = SenderBuilder {
                        peer_id: inner.peer_id,
                        track_id: track.id,
                        caps: track.media_type.into(),
                        peer: &inner.peer,
                        peer_events_sender: inner.peer_events_sender.clone(),
                        mid,
                        mute_state,
                        is_required,
                    }
                    .build()
                    .map_err(tracerr::wrap!())?;
                    inner.senders.insert(track.id, sndr);
                }
                Direction::Recv { sender, mid } => {
                    let recv = Receiver::new(
                        inner.peer_id,
                        track.id,
                        &(track.media_type.into()),
                        sender,
                        &inner.peer,
                        mid,
                        inner.peer_events_sender.clone(),
                    );
                    inner.receivers.insert(track.id, recv);
                }
            }
        }
        Ok(())
    }

    /// Updates [`Sender`]s and [`Receiver`]s of this [`super::PeerConnection`]
    /// with [`proto::TrackPatch`].
    ///
    /// # Errors
    ///
    /// Errors with [`MediaConnectionsError::InvalidTrackPatch`] if
    /// [`MediaStreamTrack`] with ID from [`proto::TrackPatch`] doesn't exist.
    pub fn patch_tracks(&self, tracks: Vec<proto::TrackPatch>) -> Result<()> {
        for track_proto in tracks {
            if let Some(sender) = self.get_sender_by_id(track_proto.id) {
                sender.update(&track_proto);
            } else if let Some(receiver) =
                self.0.borrow_mut().receivers.get_mut(&track_proto.id)
            {
                receiver.update(&track_proto);
            } else {
                return Err(tracerr::new!(
                    MediaConnectionsError::InvalidTrackPatch(track_proto.id)
                ));
            }
        }
        Ok(())
    }

    /// Returns [`StreamRequest`] if this [`MediaConnections`] has [`Sender`]s.
    pub fn get_stream_request(&self) -> Option<StreamRequest> {
        let mut stream_request = None;
        for sender in self.0.borrow().senders.values() {
            if let MuteState::Stable(StableMuteState::NotMuted) =
                sender.mute_state.get()
            {
                stream_request
                    .get_or_insert_with(StreamRequest::default)
                    .add_track_request(sender.track_id, sender.caps.clone());
            }
        }
        stream_request
    }

    /// Inserts tracks from a provided [`PeerMediaStream`] into [`Sender`]s
    /// based on track IDs.
    ///
    /// Provided [`PeerMediaStream`] must have all required
    /// [`MediaStreamTrack`]s. [`MediaStreamTrack`]s are inserted into
    /// [`Sender`]'s [`RtcRtpTransceiver`]s via [`replaceTrack` method][1],
    /// changing its direction to `sendonly`.
    ///
    /// # Errors
    ///
    /// With [`MediaConnectionsError::InvalidMediaStream`] if provided
    /// [`PeerMediaStream`] doesn't contain required [`MediaStreamTrack`].
    ///
    /// With [`MediaConnectionsError::InvalidMediaTrack`] if some
    /// [`MediaStreamTrack`] cannot be inserted into associated [`Sender`]
    /// because of constraints mismatch.
    ///
    /// With [`MediaConnectionsError::CouldNotInsertLocalTrack`] if some
    /// [`MediaStreamTrack`] from provided [`PeerMediaStream`] cannot be
    /// inserted into provided [`Sender`]s transceiver.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtpsender-replacetrack
    pub async fn insert_local_stream(
        &self,
        stream: &PeerMediaStream,
    ) -> Result<()> {
        let inner = self.0.borrow();

        // Build sender to track pairs to catch errors before inserting.
        let mut sender_and_track = Vec::with_capacity(inner.senders.len());
        for sender in inner.senders.values() {
            // skip senders that are not NotMuted
            if !sender.is_not_muted() {
                continue;
            }

            if let Some(track) = stream.get_track_by_id(sender.track_id) {
                if sender.caps.satisfies(&track) {
                    sender_and_track.push((sender, track));
                } else {
                    return Err(tracerr::new!(
                        MediaConnectionsError::InvalidMediaTrack
                    ));
                }
            } else if sender.is_required() {
                return Err(tracerr::new!(
                    MediaConnectionsError::InvalidMediaStream
                ));
            }
        }

        future::try_join_all(sender_and_track.into_iter().map(
            |(sender, track)| {
                Sender::insert_and_enable_track(Rc::clone(sender), track)
            },
        ))
        .await?;

        Ok(())
    }

    /// Adds provided [`MediaStreamTrack`] and [`RtcRtpTransceiver`] to the
    /// stored [`Receiver`], which is associated with a given
    /// [`RtcRtpTransceiver`].
    ///
    /// Returns ID of associated [`Sender`] and provided track [`TrackId`], if
    /// any.
    ///
    /// # Errors
    ///
    /// Errors with [`MediaConnectionsError::CouldNotInsertRemoteTrack`] if
    /// provided transceiver has empty `mid`, that means that negotiation has
    /// not completed.
    ///
    /// Errors with [`MediaConnectionsError::CouldNotInsertRemoteTrack`] if
    /// could not find [`Receiver`] by transceivers `mid`.
    pub fn add_remote_track(
        &self,
        transceiver: RtcRtpTransceiver,
        track: MediaStreamTrack,
    ) -> Result<()> {
        let mut inner = self.0.borrow_mut();
        if let Some(mid) = transceiver.mid() {
            for receiver in &mut inner.receivers.values_mut() {
                if let Some(recv_mid) = &receiver.mid() {
                    if recv_mid == &mid {
                        receiver.set_remote_track(transceiver, track);
                        return Ok(());
                    }
                }
            }
        }
        Err(tracerr::new!(
            MediaConnectionsError::CouldNotInsertRemoteTrack(transceiver.mid())
        ))
    }

    /// Returns [`Sender`] from this [`MediaConnections`] by [`TrackId`].
    #[inline]
    pub fn get_sender_by_id(&self, id: TrackId) -> Option<Rc<Sender>> {
        self.0.borrow().senders.get(&id).cloned()
    }

    /// Stops all [`Sender`]s state transitions expiry timers.
    pub fn stop_state_transitions_timers(&self) {
        self.0
            .borrow()
            .senders
            .values()
            .for_each(|sender| sender.stop_mute_state_transition_timeout());
    }

    /// Resets all [`Sender`]s state transitions expiry timers.
    pub fn reset_state_transitions_timers(&self) {
        self.0
            .borrow()
            .senders
            .values()
            .for_each(|sender| sender.reset_mute_state_transition_timeout());
    }
}
