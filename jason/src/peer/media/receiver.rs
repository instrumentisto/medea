//! Implementation of the `MediaTrack` with a `Recv` direction.

use std::{cell::RefCell, rc::Rc};

use futures::{channel::mpsc, future::LocalBoxFuture, StreamExt as _};
use medea_client_api_proto as proto;
use medea_client_api_proto::TrackPatch;
use medea_reactive::ObservableCell;
use proto::{PeerId, TrackId};
use wasm_bindgen_futures::spawn_local;
use web_sys::RtcRtpTransceiver;

use crate::{
    media::{MediaStreamTrack, TrackConstraints},
    peer::{
        conn::{RtcPeerConnection, TransceiverDirection, TransceiverKind},
        PeerEvent, StableMuteState,
    },
    utils::wait_for,
};

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
    pub(super) track: Rc<RefCell<Option<MediaStreamTrack>>>,
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
        peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
    ) -> Self {
        let kind = TransceiverKind::from(caps);
        let transceiver = match mid {
            None => {
                Some(peer.add_transceiver(kind, TransceiverDirection::Recvonly))
            }
            Some(_) => None,
        };
        let mute_state = ObservableCell::new(StableMuteState::NotMuted);
        let mut mute_state_changes = mute_state.subscribe();
        let track = Rc::new(RefCell::new(None));
        spawn_local({
            let track = Rc::clone(&track);
            async move {
                while let Some(mute_state_update) =
                    mute_state_changes.next().await
                {
                    fn get_track(
                        track: Rc<RefCell<Option<MediaStreamTrack>>>,
                    ) -> LocalBoxFuture<'static, MediaStreamTrack>
                    {
                        Box::pin(async move {
                            let inner_track =
                                { track.borrow().as_ref().cloned() };
                            if let Some(track) = inner_track {
                                track
                            } else {
                                wait_for(
                                    |track| track.borrow().is_some(),
                                    Rc::clone(&track),
                                )
                                .await;

                                get_track(track).await
                            }
                        })
                    }

                    let track = get_track(Rc::clone(&track)).await;
                    if peer_events_sender
                        .unbounded_send(PeerEvent::MuteStateChanged {
                            peer_id: sender_id,
                            track,
                            mute_state: mute_state_update,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }
        });

        Self {
            track_id,
            sender_id,
            transceiver,
            mid,
            track,
            mute_state,
        }
    }

    /// Updates [`Receiver`] with a provided [`TrackPatch`].
    pub fn update(&self, track_patch: &TrackPatch) {
        if let Some(is_muted) = track_patch.is_muted {
            self.mute_state.set(is_muted.into());
        }
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
