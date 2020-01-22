//! [MediaStreamTrack][1] representation.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack

use std::{cell::RefCell, ops::Not, rc::Rc};

use futures::StreamExt;
use medea_client_api_proto::{self as proto, TrackId as Id};
use medea_reactive::{Dropped, Reactive};
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;
use web_sys::MediaStreamTrack;

use crate::media::TrackConstraints;

/// Mute state of [`MediaTrack`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MutedState {
    /// [`MediaTrack`] is unmuted.
    Unmuted,

    /// [`MediaTrack`] should be unmuted, but awaits server permission.
    Unmuting,

    /// [`MediaTrack`] should be muted, but awaits server permission.
    Muting,

    /// [`MediaTrack`] is muted.
    Muted,
}

impl MutedState {
    /// Returns [`MutedState`] which should be set while transition to this
    /// [`MutedState`].
    pub fn proccessing_state(self) -> Self {
        match self {
            Self::Unmuted => Self::Unmuting,
            Self::Muted => Self::Muting,
            _ => self,
        }
    }
}

impl From<bool> for MutedState {
    fn from(is_muted: bool) -> Self {
        if is_muted {
            Self::Muted
        } else {
            Self::Unmuted
        }
    }
}

impl Not for MutedState {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Self::Muted => Self::Unmuted,
            Self::Unmuted => Self::Muted,
            Self::Unmuting => Self::Muting,
            Self::Muting => Self::Unmuting,
        }
    }
}

/// Representation of [MediaStreamTrack][1].
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
pub struct MediaTrack {
    id: Id,
    track: MediaStreamTrack,
    caps: TrackConstraints,
    muted_state: RefCell<Reactive<MutedState>>,
}

impl MediaTrack {
    /// Instantiates new [`MediaTrack`].
    pub fn new(
        id: Id,
        track: MediaStreamTrack,
        caps: TrackConstraints,
        muted_state: MutedState,
    ) -> Rc<Self> {
        let muted_state = RefCell::new(Reactive::new(muted_state));
        let mut muted_state_subscribe = muted_state.borrow().subscribe();
        let this = Rc::new(Self {
            id,
            track,
            caps,
            muted_state,
        });

        let this_weak = Rc::downgrade(&this);
        spawn_local(async move {
            while let Some(changed_muted_state) =
                muted_state_subscribe.next().await
            {
                if let Some(this) = this_weak.upgrade() {
                    match changed_muted_state {
                        MutedState::Muted => {
                            this.track.set_enabled(false);
                        }
                        MutedState::Unmuted => {
                            this.track.set_enabled(true);
                        }
                        _ => (),
                    }
                } else {
                    break;
                }
            }
        });

        this
    }

    /// Returns ID of this [`MediaTrack`].
    pub fn id(&self) -> Id {
        self.id
    }

    /// Returns the underlying [`MediaStreamTrack`] object of this
    /// [`MediaTrack`].
    pub fn track(&self) -> &MediaStreamTrack {
        &self.track
    }

    /// Returns [`MediaType`] of this [`MediaTrack`].
    pub fn caps(&self) -> &TrackConstraints {
        &self.caps
    }

    /// Returns current [`MutedState`] of this [`MediaTrack`].
    pub fn get_muted_state(&self) -> MutedState {
        **self.muted_state.borrow()
    }

    /// Changes [`MutedState`] of this [`MediaTrack`].
    pub fn change_muted_state(&self, new_state: MutedState) {
        *self.muted_state.borrow_mut().borrow_mut() = new_state;
    }

    /// Will be resolved when [`MutedState`] of this [`Track`] will be become
    /// equal to provided [`MutedState`].
    pub async fn on_muted_state(
        &self,
        state: MutedState,
    ) -> Result<(), Traced<Dropped>> {
        let subscription = self.muted_state.borrow().when_eq(state);
        subscription.await.map_err(|e| tracerr::new!(e))
    }

    /// Update this [`Track`] based on provided
    /// [`medea_client_api_proto::TrackUpdate`].
    pub fn update(&self, track: &proto::TrackUpdate) {
        if let Some(is_muted) = track.is_muted {
            if is_muted {
                *self.muted_state.borrow_mut().borrow_mut() = MutedState::Muted;
            } else {
                *self.muted_state.borrow_mut().borrow_mut() =
                    MutedState::Unmuted;
            }
        }
    }
}
