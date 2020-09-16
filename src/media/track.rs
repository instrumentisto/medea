//! Remote [MediaStreamTrack][1] representation.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack

use std::cell::{Cell, RefCell};

use medea_client_api_proto::{MediaType, TrackId as Id};

/// Representation of [MediaStreamTrack][1] object.
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
#[derive(Debug)]
pub struct MediaTrack {
    pub id: Id,
    mid: RefCell<Option<String>>,
    pub media_type: MediaType,
    enabled: Cell<bool>,
    mute_state: RefCell<MuteState>,
}

impl MediaTrack {
    /// Creates new [`MediaTrack`] of the specified [`MediaType`].
    pub fn new(id: Id, media_type: MediaType) -> Self {
        Self {
            id,
            mid: RefCell::new(None),
            media_type,
            enabled: Cell::new(true),
            mute_state: RefCell::new(MuteState::new()),
        }
    }

    pub fn set_mid(&self, mid: String) {
        self.mid.borrow_mut().replace(mid);
    }

    pub fn mid(&self) -> Option<String> {
        self.mid.borrow_mut().as_ref().cloned()
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.get()
    }

    /// Returns `true` if this [`MediaTrack`] currently is muted.
    pub fn is_muted(&self) -> bool {
        self.mute_state.borrow().is_muted()
    }

    /// Sets mute state of the [`MediaTrack`]'s `Recv` side.
    pub fn set_recv_mute_state(&self, is_muted: bool) {
        self.mute_state.borrow_mut().set_recv(is_muted);
    }

    /// Sets mute state of the [`MediaTrack`]'s `Send` side.
    pub fn set_send_mute_state(&self, is_muted: bool) {
        self.mute_state.borrow_mut().set_send(is_muted);
    }
}

/// Mute state of the [`MediaTrack`].
///
/// Contains mute state for the `Send` and `Recv` side.
#[derive(Clone, Copy, Debug)]
struct MuteState {
    /// Mute state of the `Send` side.
    ///
    /// If `true` then sender is muted.
    send_muted: bool,

    /// Mute state of the `Recv` side.
    ///
    /// If `true` then receiver is muted.
    recv_muted: bool,
}

impl Default for MuteState {
    fn default() -> Self {
        Self {
            send_muted: false,
            recv_muted: false,
        }
    }
}

impl MuteState {
    /// Returns new default [`MuteState`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if [`MuteState::send_muted`] or [`MuteState::recv_muted`]
    /// are `true`.
    pub fn is_muted(self) -> bool {
        self.send_muted || self.recv_muted
    }

    /// Sets mute state for the `Recv` side of [`MediaTrack`].
    pub fn set_recv(&mut self, is_muted: bool) {
        self.recv_muted = is_muted;
    }

    /// Sets mute state for the `Send` side of the [`MediaTrack`].
    pub fn set_send(&mut self, is_muted: bool) {
        self.send_muted = is_muted;
    }
}
