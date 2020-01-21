//! [MediaStreamTrack][1] representation.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack

use std::{cell::RefCell, rc::Rc};

use futures::StreamExt;
use medea_client_api_proto::{self as proto, TrackId as Id};
use medea_reactive::{Dropped, Reactive};
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;
use web_sys::MediaStreamTrack;

use crate::media::TrackConstraints;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MutedState {
    Unmuted,
    Unmuting,
    Muting,
    Muted,
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

        // TODO: this Future will hold [`MediaTrack`] from dropping. Should be
        // fixed.
        let this_clone = this.clone();
        spawn_local(async move {
            while let Some(changed_muted_state) =
                muted_state_subscribe.next().await
            {
                match changed_muted_state {
                    MutedState::Muted => {
                        this_clone.track.set_enabled(false);
                    }
                    MutedState::Unmuted => {
                        this_clone.track.set_enabled(true);
                    }
                    _ => (),
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

    pub fn get_muted_state(&self) -> MutedState {
        **self.muted_state.borrow()
    }

    pub fn change_muted_state(&self, new_state: MutedState) {
        *self.muted_state.borrow_mut().borrow_mut() = new_state;
    }

    pub async fn on_muted_state(
        &self,
        state: MutedState,
    ) -> Result<(), Traced<Dropped>> {
        let subscription = self.muted_state.borrow().when_eq(state);
        subscription.await.map_err(|e| tracerr::new!(e))
    }

    pub fn update(&self, track: proto::TrackUpdate) {
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
