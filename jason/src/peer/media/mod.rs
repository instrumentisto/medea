//! [`crate::peer::PeerConnection`] media management.

mod mute_state;
mod receiver;
mod sender;

use std::{cell::RefCell, collections::HashMap, convert::From, rc::Rc};

use derive_more::Display;
use futures::{channel::mpsc, future, future::LocalBoxFuture};
use medea_client_api_proto as proto;
use medea_reactive::DroppedError;
use proto::{Direction, PeerId, TrackId};
use tracerr::Traced;
use web_sys::{MediaStreamTrack as SysMediaStreamTrack, RtcRtpTransceiver};

use crate::{
    media::{LocalStreamConstraints, RecvConstraints},
    peer::PeerEvent,
    utils::{JsCaused, JsError},
};

use super::{
    conn::{RtcPeerConnection, TransceiverKind},
    stream::PeerMediaStream,
    stream_request::StreamRequest,
};

use self::{mute_state::MuteStateController, sender::SenderBuilder};

pub use self::{
    mute_state::{MuteState, MuteStateTransition, StableMuteState},
    receiver::Receiver,
    sender::Sender,
};

/// Transceiver's sending ([`Sender`]) or receiving ([`Receiver`]) side.
pub trait TransceiverSide: Muteable {
    /// Returns [`TrackId`] of this [`TransceiverSide`].
    fn track_id(&self) -> TrackId;

    /// Returns [`TransceiverKind`] of this [`TransceiverSide`].
    fn kind(&self) -> TransceiverKind;

    /// Returns [`TransceiverKind`] of this [`TransceiverSide`].
    fn mid(&self) -> Option<String>;
}

/// Default functions for dealing with [`MuteStateController`] for objects that
/// use it.
pub trait Muteable {
    /// Returns reference to the [`MuteStateController`].
    fn mute_state_controller(&self) -> Rc<MuteStateController>;

    /// Returns [`MuteState`] of this [`Muteable`].
    #[inline]
    fn mute_state(&self) -> MuteState {
        self.mute_state_controller().mute_state()
    }

    /// Sets current [`MuteState`] to [`MuteState::Transition`].
    ///
    /// # Errors
    ///
    /// Implementors might return [`MediaConnectionsError`] if transition could
    /// not be made for some reason.
    #[inline]
    fn mute_state_transition_to(
        &self,
        desired_state: StableMuteState,
    ) -> Result<()> {
        self.mute_state_controller().transition_to(desired_state);

        Ok(())
    }

    /// Cancels [`MuteState`] transition.
    #[inline]
    fn cancel_transition(&self) {
        self.mute_state_controller().cancel_transition()
    }

    /// Returns [`Future`] which will be resolved when [`MuteState`] of this
    /// [`Muteable`] will be [`MuteState::Stable`] or it is dropped.
    ///
    /// # Errors
    ///
    /// [`MediaConnectionsError::MuteStateTransitsIntoOppositeState`] is
    /// returned if [`MuteState`] transits into the opposite to the
    /// `desired_state`.
    #[inline]
    fn when_mute_state_stable(
        &self,
        desired_state: StableMuteState,
    ) -> LocalBoxFuture<'static, Result<()>> {
        self.mute_state_controller()
            .when_mute_state_stable(desired_state)
    }

    /// Stops state transition timer of this [`Muteable`].
    #[inline]
    fn stop_mute_state_transition_timeout(&self) {
        self.mute_state_controller().stop_transition_timeout()
    }

    /// Resets state transition timer of this [`Muteable`].
    #[inline]
    fn reset_mute_state_transition_timeout(&self) {
        self.mute_state_controller().reset_transition_timeout()
    }

    /// Indicates whether mute state of the [`Muteable`] is in
    /// [`MuteState::Muted`].
    #[inline]
    fn is_muted(&self) -> bool {
        self.mute_state_controller().is_muted()
    }

    /// Indicates whether mute state of the [`Muteable`] is in
    /// [`MuteState::Unmuted`].
    #[inline]
    fn is_unmuted(&self) -> bool {
        self.mute_state_controller().is_unmuted()
    }
}

/// Direction of the `MediaTrack`.
#[derive(Debug, Clone, Copy)]
pub enum TrackDirection {
    /// Sends media data.
    Send,

    /// Receives media data.
    Recv,
}

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
    receivers: HashMap<TrackId, Rc<Receiver>>,
}

impl InnerMediaConnections {
    /// Returns [`Iterator`] over [`Sender`]s with provided [`TransceiverKind`].
    fn iter_senders_with_kind(
        &self,
        kind: TransceiverKind,
    ) -> impl Iterator<Item = &Rc<Sender>> {
        self.senders.values().filter(move |s| s.kind() == kind)
    }

    /// Returns [`Iterator`] over [`Receiver`]s with provided
    /// [`TransceiverKind`].
    fn iter_receivers_with_kind(
        &self,
        kind: TransceiverKind,
    ) -> impl Iterator<Item = &Rc<Receiver>> {
        self.receivers.values().filter(move |s| s.kind() == kind)
    }

    /// Returns all [`TransceiverSide`]s by provided [`TrackDirection`] and
    /// [`TransceiverKind`].
    fn get_transceivers_by_direction_and_kind(
        &self,
        direction: TrackDirection,
        kind: TransceiverKind,
    ) -> Vec<Rc<dyn TransceiverSide>> {
        match direction {
            TrackDirection::Send => self
                .iter_senders_with_kind(kind)
                .map(|tx| Rc::clone(&tx) as Rc<dyn TransceiverSide>)
                .collect(),
            TrackDirection::Recv => self
                .iter_receivers_with_kind(kind)
                .map(|rx| Rc::clone(&rx) as Rc<dyn TransceiverSide>)
                .collect(),
        }
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

    /// Returns all [`Sender`]s and [`Receiver`]s from this [`MediaConnections`]
    /// with provided [`TransceiverKind`] and [`TrackDirection`].
    pub fn get_transceivers_sides(
        &self,
        kind: TransceiverKind,
        direction: TrackDirection,
    ) -> Vec<Rc<dyn TransceiverSide>> {
        self.0
            .borrow()
            .get_transceivers_by_direction_and_kind(direction, kind)
    }

    /// Returns `true` if all [`TransceiverSide`]s with provided
    /// [`TransceiverKind`] and [`TrackDirection`] is in provided [`MuteState`].
    pub fn is_all_tracks_in_mute_state(
        &self,
        kind: TransceiverKind,
        direction: TrackDirection,
        mute_state: StableMuteState,
    ) -> bool {
        let transceivers = self
            .0
            .borrow()
            .get_transceivers_by_direction_and_kind(direction, kind);
        for transceiver in transceivers {
            if transceiver.mute_state() != mute_state.into() {
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

    /// Returns `true` if all [`Receiver`]s with [`TransceiverKind::Video`] are
    /// enabled or `false` otherwise.
    pub fn is_recv_video_enabled(&self) -> bool {
        self.0
            .borrow()
            .iter_receivers_with_kind(TransceiverKind::Video)
            .find(|s| s.is_muted())
            .is_none()
    }

    /// Returns `true` if all [`Receiver`]s with [`TransceiverKind::Audio`] are
    /// enabled or `false` otherwise.
    pub fn is_recv_audio_enabled(&self) -> bool {
        self.0
            .borrow()
            .iter_receivers_with_kind(TransceiverKind::Audio)
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
        let inner = self.0.borrow();
        let mut mids =
            HashMap::with_capacity(inner.senders.len() + inner.receivers.len());
        for (track_id, sender) in &inner.senders {
            mids.insert(
                *track_id,
                sender
                    .mid()
                    .ok_or(MediaConnectionsError::SendersWithoutMid)
                    .map_err(tracerr::wrap!())?,
            );
        }
        for (track_id, receiver) in &inner.receivers {
            mids.insert(
                *track_id,
                receiver
                    .mid()
                    .ok_or(MediaConnectionsError::ReceiversWithoutMid)
                    .map_err(tracerr::wrap!())?,
            );
        }
        Ok(mids)
    }

    /// Returns activity statuses of the all [`Sender`]s and [`Receiver`]s from
    /// this [`MediaConnections`].
    pub fn get_transceivers_statuses(&self) -> HashMap<TrackId, bool> {
        let inner = self.0.borrow();

        let mut out = HashMap::new();
        for (track_id, sender) in &inner.senders {
            out.insert(*track_id, sender.is_publishing());
        }
        for (track_id, receiver) in &inner.receivers {
            out.insert(*track_id, receiver.is_receiving());
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
    pub fn create_tracks<I: IntoIterator<Item = proto::Track>>(
        &self,
        tracks: I,
        send_constraints: &LocalStreamConstraints,
        recv_constraints: &RecvConstraints,
    ) -> Result<()> {
        let mut inner = self.0.borrow_mut();
        for track in tracks {
            let is_required = track.is_required();
            match track.direction {
                Direction::Send { mid, .. } => {
                    let mute_state;
                    if send_constraints.is_enabled(&track.media_type) {
                        mute_state = StableMuteState::Unmuted;
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
                    let recv = Rc::new(Receiver::new(
                        track.id,
                        track.media_type.into(),
                        sender,
                        &inner.peer,
                        mid,
                        inner.peer_events_sender.clone(),
                        recv_constraints,
                    ));
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
    pub fn patch_tracks(
        &self,
        tracks: Vec<proto::TrackPatchEvent>,
    ) -> Result<()> {
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
            if let MuteState::Stable(StableMuteState::Unmuted) =
                sender.mute_state()
            {
                stream_request
                    .get_or_insert_with(StreamRequest::default)
                    .add_track_request(
                        sender.track_id(),
                        sender.caps().clone(),
                    );
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
            // skip senders that are not Unmuted
            if !sender.is_unmuted() {
                continue;
            }

            if let Some(track) = stream.get_track_by_id(sender.track_id()) {
                if sender.caps().satisfies(&track) {
                    sender_and_track.push((sender, track));
                } else {
                    return Err(tracerr::new!(
                        MediaConnectionsError::InvalidMediaTrack
                    ));
                }
            } else if sender.caps().is_required() {
                return Err(tracerr::new!(
                    MediaConnectionsError::InvalidMediaStream
                ));
            }
        }

        future::try_join_all(sender_and_track.into_iter().map(
            |(sender, track)| async move {
                Rc::clone(sender).insert_track(track).await?;
                sender.maybe_enable();
                Ok::<(), Traced<MediaConnectionsError>>(())
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
        track: SysMediaStreamTrack,
    ) -> Result<()> {
        let inner = self.0.borrow();
        if let Some(mid) = transceiver.mid() {
            for receiver in inner.receivers.values() {
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

    /// Returns all references to the [`TransceiverSide`]s from this
    /// [`MediaConnections`].
    fn get_all_transceivers_sides(&self) -> Vec<Rc<dyn TransceiverSide>> {
        let inner = self.0.borrow();
        inner
            .senders
            .values()
            .map(|s| Rc::clone(s) as Rc<dyn TransceiverSide>)
            .chain(
                inner
                    .receivers
                    .values()
                    .map(|r| Rc::clone(&r) as Rc<dyn TransceiverSide>),
            )
            .collect()
    }

    /// Stops all [`TransceiverSide`]s state transitions expiry timers.
    pub fn stop_state_transitions_timers(&self) {
        self.get_all_transceivers_sides()
            .into_iter()
            .for_each(|t| t.stop_mute_state_transition_timeout())
    }

    /// Resets all [`TransceiverSide`]s state transitions expiry timers.
    pub fn reset_state_transitions_timers(&self) {
        self.get_all_transceivers_sides()
            .into_iter()
            .for_each(|t| t.reset_mute_state_transition_timeout());
    }
}
