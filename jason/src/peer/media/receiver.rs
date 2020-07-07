//! Implementation of the `MediaTrack` with a `Recv` direction.

use medea_client_api_proto as proto;
use proto::{PeerId, TrackId};
use web_sys::RtcRtpTransceiver;

use crate::{
    media::{MediaStreamTrack, TrackConstraints},
    peer::{
        conn::{RtcPeerConnection, TransceiverDirection, TransceiverKind},
        StableMuteState,
    },
    utils::console_error,
};
use futures::{stream::LocalBoxStream, Stream, StreamExt};
use medea_client_api_proto::TrackPatch;
use medea_reactive::ObservableCell;
use wasm_bindgen_futures::spawn_local;

/// Representation of a remote [`MediaStreamTrack`] that is being received from
/// some remote peer. It may have two states: `waiting` and `receiving`.
///
/// We can save related [`RtcRtpTransceiver`] and the actual
/// [`MediaStreamTrack`] only when [`MediaStreamTrack`] data arrives.
pub struct Receiver {
    pub(super) track_id: TrackId,
    pub(super) sender_id: PeerId,
    pub(super) transceiver: Option<RtcRtpTransceiver>,
    pub(super) mid: Option<String>,
    pub(super) track: Option<MediaStreamTrack>,
    pub(super) mute_state: ObservableCell<StableMuteState>,
}

impl Receiver {
    /// Creates new [`RtcRtpTransceiver`] if provided `mid` is `None`, otherwise
    /// creates [`Receiver`] without [`RtcRtpTransceiver`]. It will be injected
    /// when [`MediaStreamTrack`] arrives.
    ///
    /// `track` field in the created [`Receiver`] will be `None`, since
    /// [`Receiver`] must be created before the actual [`MediaStreamTrack`] data
    /// arrives.
    pub(super) fn new(
        track_id: TrackId,
        caps: &TrackConstraints,
        sender_id: PeerId,
        peer: &RtcPeerConnection,
        mid: Option<String>,
    ) -> Self {
        let kind = TransceiverKind::from(caps);
        let transceiver = match mid {
            None => {
                Some(peer.add_transceiver(kind, TransceiverDirection::Recvonly))
            }
            Some(_) => None,
        };

        Self {
            track_id,
            sender_id,
            transceiver,
            mid,
            track: None,
            mute_state: ObservableCell::new(StableMuteState::NotMuted),
        }
    }

    pub fn update(&self, track_patch: &TrackPatch) {
        if let Some(is_muted) = track_patch.is_muted {
            self.mute_state.set(is_muted.into());
        }
    }

    pub fn on_mute_state_update(
        &self,
    ) -> LocalBoxStream<'static, StableMuteState> {
        self.mute_state.subscribe()
    }

    /// Returns `mid` of this [`Receiver`].
    ///
    /// Tries to fetch it from the underlying [`RtcRtpTransceiver`] if current
    /// value is `None`.
    pub(crate) fn mid(&mut self) -> Option<&str> {
        if self.mid.is_none() && self.transceiver.is_some() {
            self.mid = self.transceiver.as_ref().unwrap().mid()
        }
        self.mid.as_deref()
    }
}
