//! Remote [MediaStreamTrack][1] representation.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack

use std::cell::{Cell, RefCell};

use medea_client_api_proto::{MediaType, TrackId as Id};

/// Representation of [MediaStreamTrack][1] object.
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
#[derive(Debug, Eq, PartialEq)]
pub struct MediaTrack {
    pub id: Id,
    mid: RefCell<Option<String>>,
    pub media_type: MediaType,
    transceiver_enabled: Cell<bool>,
    media_exchange_state: RefCell<MediaExchangeState>,
}

impl MediaTrack {
    /// Creates new [`MediaTrack`] of the specified [`MediaType`].
    pub fn new(id: Id, media_type: MediaType) -> Self {
        Self {
            id,
            mid: RefCell::new(None),
            media_type,
            transceiver_enabled: Cell::new(true),
            media_exchange_state: RefCell::new(MediaExchangeState::new()),
        }
    }

    pub fn set_mid(&self, mid: String) {
        self.mid.borrow_mut().replace(mid);
    }

    pub fn mid(&self) -> Option<String> {
        self.mid.borrow_mut().as_ref().cloned()
    }

    pub fn set_transceiver_enabled(&self, enabled: bool) {
        self.transceiver_enabled.set(enabled);
    }

    pub fn is_transceiver_enabled(&self) -> bool {
        self.transceiver_enabled.get()
    }

    /// Returns `true` if this [`MediaTrack`] currently is disabled.
    pub fn is_media_exchange_disabled(&self) -> bool {
        self.media_exchange_state.borrow().is_disabled()
    }

    /// Sets media exchange state of the [`MediaTrack`]'s `Recv` side.
    pub fn set_recv_media_exchange_state(&self, is_disabled: bool) {
        self.media_exchange_state.borrow_mut().set_recv(is_disabled);
    }

    /// Sets media exchange state of the [`MediaTrack`]'s `Send` side.
    pub fn set_send_media_exchange_state(&self, is_disabled: bool) {
        self.media_exchange_state.borrow_mut().set_send(is_disabled);
    }
}

/// media exchange state of the [`MediaTrack`].
///
/// Contains media exchange state for the `Send` and `Recv` side.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MediaExchangeState {
    /// Media exchange state of the `Send` side.
    ///
    /// If `true` then sender is disabled.
    send_disabled: bool,

    /// Media exchange state of the `Recv` side.
    ///
    /// If `true` then receiver is disabled.
    recv_disabled: bool,
}

impl Default for MediaExchangeState {
    fn default() -> Self {
        Self {
            send_disabled: false,
            recv_disabled: false,
        }
    }
}

impl MediaExchangeState {
    /// Returns new default [`MediaExchangeState`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if [`MediaExchangeState::send_disabled`] or
    /// [`MediaExchangeState::recv_disabled`] are `true`.
    pub fn is_disabled(self) -> bool {
        self.send_disabled || self.recv_disabled
    }

    /// Sets media exchange state for the `Recv` side of [`MediaTrack`].
    pub fn set_recv(&mut self, is_disabled: bool) {
        self.recv_disabled = is_disabled;
    }

    /// Sets media exchange state for the `Send` side of the [`MediaTrack`].
    pub fn set_send(&mut self, is_disabled: bool) {
        self.send_disabled = is_disabled;
    }
}
