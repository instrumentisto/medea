//! Implementation of the `MediaTrack` with a `Send` direction.

use std::{cell::Cell, rc::Rc};

use medea_client_api_proto::{MediaSourceKind, TrackId, TrackPatchEvent};

use crate::{
    media::{
        LocalTracksConstraints, MediaKind, MediaStreamTrack, TrackConstraints,
        VideoSource,
    },
    peer::{
        media::TransceiverSide,
        transceiver::{Transceiver, TransceiverDirection},
    },
};

use super::{
    mute_state::{MuteStateController, StableMuteState},
    MediaConnections, MediaConnectionsError, Muteable, Result,
};

/// Builder of the [`Sender`].
pub struct SenderBuilder<'a> {
    pub media_connections: &'a MediaConnections,
    pub track_id: TrackId,
    pub caps: TrackConstraints,
    pub mid: Option<String>,
    pub mute_state: StableMuteState,
    pub is_required: bool,
    pub send_constraints: LocalTracksConstraints,
}

impl<'a> SenderBuilder<'a> {
    /// Builds new [`RtcRtpTransceiver`] if provided `mid` is `None`, otherwise
    /// retrieves existing [`RtcRtpTransceiver`] via provided `mid` from a
    /// provided [`RtcPeerConnection`]. Errors if [`RtcRtpTransceiver`] lookup
    /// fails.
    pub fn build(self) -> Result<Rc<Sender>> {
        let connections = self.media_connections.0.borrow();
        let kind = MediaKind::from(&self.caps);
        let transceiver = match self.mid {
            // Try to find rcvr transceiver that can be used as sendrecv.
            None => connections
                .receivers
                .values()
                .find(|rcvr| {
                    rcvr.caps().media_kind() == self.caps.media_kind()
                        && rcvr.caps().media_source_kind()
                            == self.caps.media_source_kind()
                })
                .and_then(|rcvr| rcvr.transceiver())
                .unwrap_or_else(|| {
                    connections
                        .add_transceiver(kind, TransceiverDirection::INACTIVE)
                }),
            Some(mid) => connections
                .get_transceiver_by_mid(&mid)
                .ok_or(MediaConnectionsError::TransceiverNotFound(mid))
                .map_err(tracerr::wrap!())?,
        };

        let mute_state_observer = MuteStateController::new(self.mute_state);
        let this = Rc::new(Sender {
            track_id: self.track_id,
            caps: self.caps,
            general_mute_state: Cell::new(self.mute_state),
            transceiver,
            mute_state: mute_state_observer,
            is_required: self.is_required,
            send_constraints: self.send_constraints,
        });

        Ok(this)
    }
}

/// Representation of a local [`MediaStreamTrack`] that is being sent to some
/// remote peer.
pub struct Sender {
    track_id: TrackId,
    caps: TrackConstraints,
    transceiver: Transceiver,
    mute_state: Rc<MuteStateController>,
    general_mute_state: Cell<StableMuteState>,
    is_required: bool,
    send_constraints: LocalTracksConstraints,
}

impl Sender {
    /// Returns [`TrackConstraints`] of this [`Sender`].
    #[inline]
    pub fn caps(&self) -> &TrackConstraints {
        &self.caps
    }

    /// Returns `true` if this [`Sender`] is publishing media traffic.
    pub fn is_publishing(&self) -> bool {
        self.transceiver.has_direction(TransceiverDirection::SEND)
    }

    /// Updates [`Sender`]s general mute state based on the provided
    /// [`StableMuteState`].
    ///
    /// Sets [`Sender`]s underlying transceiver direction to
    /// [`TransceiverDirection::Inactive`] if provided mute state is
    /// [`StableMuteState::Muted`].
    ///
    /// Emits [`PeerEvent::NewLocalStreamRequired`] if new state is
    /// [`StableMuteState::Unmuted`] and [`Sender`] does not have a track to
    /// send.
    fn update_general_mute_state(&self, mute_state: StableMuteState) {
        if self.general_mute_state.get() != mute_state {
            self.general_mute_state.set(mute_state);
            match mute_state {
                StableMuteState::Unmuted => {
                    if self.is_enabled_in_cons() {
                        self.transceiver
                            .add_direction(TransceiverDirection::SEND);
                    }
                }
                StableMuteState::Muted => {
                    self.transceiver.sub_direction(TransceiverDirection::SEND);
                }
            }
        }
    }

    /// Returns `true` if [`Transceiver`] of this [`Sender`] has
    /// [`MediaStreamTrack`].
    #[inline]
    pub fn has_track(&self) -> bool {
        self.transceiver.has_send_track()
    }

    /// Inserts provided [`MediaStreamTrack`] into provided [`Sender`]s
    /// transceiver. No-op if provided track already being used by this
    /// [`Sender`].
    pub(super) async fn insert_track(
        self: Rc<Self>,
        new_track: MediaStreamTrack,
    ) -> Result<()> {
        // no-op if we try to insert same track
        if let Some(current_track) = self.transceiver.send_track() {
            if new_track.id() == current_track.id() {
                return Ok(());
            }
        }

        self.transceiver
            .set_send_track(Some(new_track))
            .await
            .map_err(Into::into)
            .map_err(MediaConnectionsError::CouldNotInsertLocalTrack)
            .map_err(tracerr::wrap!())?;

        Ok(())
    }

    /// Indicates whether this [`Sender`] is enabled in
    /// [`LocalStreamConstraints`].
    fn is_enabled_in_cons(&self) -> bool {
        self.send_constraints.is_track_enabled(
            self.caps.media_kind(),
            self.caps.media_source_kind(),
        )
    }

    /// Updates this [`Sender`]s tracks based on the provided
    /// [`TrackPatchEvent`].
    pub async fn update(&self, track: &TrackPatchEvent) {
        if track.id != self.track_id {
            return;
        }

        if let Some(is_muted) = track.is_muted_individual {
            self.mute_state.update(is_muted);
            if is_muted {
                self.remove_track().await;
            }
        }
        if let Some(is_muted_general) = track.is_muted_general {
            self.update_general_mute_state(is_muted_general.into());
        }
    }

    /// Changes underlying transceiver direction to
    /// [`TransceiverDirection::Sendonly`] if this [`Receiver`]s general mute
    /// state is [`StableMuteState::Unmuted`].
    pub fn maybe_enable(&self) {
        if self.is_general_unmuted()
            && !self.transceiver.has_direction(TransceiverDirection::SEND)
            && self.is_enabled_in_cons()
        {
            self.transceiver.add_direction(TransceiverDirection::SEND);
        }
    }

    /// Returns [`Transceiver`] of this [`Sender`].
    pub fn transceiver(&self) -> Transceiver {
        self.transceiver.clone()
    }

    /// Checks whether general mute state of the [`Sender`] is in
    /// [`StableMuteState::Muted`].
    #[cfg(feature = "mockable")]
    pub fn is_general_muted(&self) -> bool {
        self.general_mute_state.get() == StableMuteState::Muted
    }

    /// Checks whether general mute state of the [`Sender`] is in
    /// [`MuteState::Unmuted`].
    fn is_general_unmuted(&self) -> bool {
        self.general_mute_state.get() == StableMuteState::Unmuted
    }

    /// Drops [`MediaStreamTrack`] used by this [`Sender`]. Sets track used by
    /// sending side of inner transceiver to `None`.
    async fn remove_track(&self) {
        // cannot fail
        self.transceiver.set_send_track(None).await.unwrap();
    }
}

impl TransceiverSide for Sender {
    fn track_id(&self) -> TrackId {
        self.track_id
    }

    fn kind(&self) -> MediaKind {
        MediaKind::from(&self.caps)
    }

    fn source_kind(&self) -> MediaSourceKind {
        self.caps.media_source_kind()
    }

    fn mid(&self) -> Option<String> {
        self.transceiver.mid()
    }

    fn is_transitable(&self) -> bool {
        match &self.caps {
            TrackConstraints::Video(VideoSource::Device(_)) => {
                self.send_constraints.inner().get_device_video().is_some()
            }
            TrackConstraints::Video(VideoSource::Display(_)) => {
                self.send_constraints.inner().get_display_video().is_some()
            }
            TrackConstraints::Audio(_) => true,
        }
    }
}

impl Muteable for Sender {
    #[inline]
    fn mute_state_controller(&self) -> Rc<MuteStateController> {
        self.mute_state.clone()
    }

    /// Sets current [`MuteState`] to [`MuteState::Transition`].
    ///
    /// # Errors
    ///
    /// [`MediaConnectionsError::SenderIsRequired`] is returned if [`Sender`] is
    /// required for the call and can't be muted.
    fn mute_state_transition_to(
        &self,
        desired_state: StableMuteState,
    ) -> Result<()> {
        if self.is_required {
            Err(tracerr::new!(
                MediaConnectionsError::CannotDisableRequiredSender
            ))
        } else {
            self.mute_state.transition_to(desired_state);
            Ok(())
        }
    }
}
