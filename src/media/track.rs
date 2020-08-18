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

#[derive(Clone, Copy, Debug)]
struct MuteState {
    send_muted: bool,
    recv_muted: bool,
}

impl MuteState {
    pub fn new() -> Self {
        Self {
            send_muted: false,
            recv_muted: false,
        }
    }

    pub fn is_muted(self) -> bool {
        self.send_muted || self.recv_muted
    }

    pub fn set_recv(&mut self, is_muted: bool) {
        self.recv_muted = is_muted;
    }

    pub fn set_send(&mut self, is_muted: bool) {
        self.send_muted = is_muted;
    }
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

    pub fn is_muted(&self) -> bool {
        self.mute_state.borrow().is_muted()
    }

    pub fn set_recv_mute_state(&self, is_muted: bool) {
        self.mute_state.borrow_mut().set_recv(is_muted);
    }

    pub fn set_send_mute_state(&self, is_muted: bool) {
        self.mute_state.borrow_mut().set_send(is_muted);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_enabled() {
        for (send, recv, result) in &[
            (false, false, false),
            (true, false, true),
            (true, true, true),
            (false, true, true),
        ] {
            let mut state = MuteState::new();
            state.set_send(*send);
            state.set_recv(*recv);

            assert_eq!(state.is_muted(), *result);
        }
    }
}
