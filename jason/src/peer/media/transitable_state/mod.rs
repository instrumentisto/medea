//! [`MediaStateControllable`]s media exchange state.
//!
//! [`MediaStateControllable`]: crate::peer::MediaStateControllable

mod controller;
pub mod media_exchange_state;
pub mod mute_state;

use derive_more::From;
use medea_client_api_proto::{TrackId, TrackPatchCommand};

#[doc(inline)]
pub use self::controller::{
    MediaExchangeStateController, MuteStateController,
    TransitableStateController,
};

/// [`TransitableState`] for the [`media_exchange_state::Stable`].
pub type MediaExchangeState = TransitableState<
    media_exchange_state::Stable,
    media_exchange_state::Transition,
>;
/// [`TransitableState`] for the [`mute_state::Stable`].
pub type MuteState =
    TransitableState<mute_state::Stable, mute_state::Transition>;

/// All media states which can be toggled in the [`MediaStateControllable`].
///
/// [`MediaStateControllable`]: crate::peer::MediaStateControllable
#[derive(Clone, Copy, Debug, From)]
pub enum MediaState {
    /// Responsible for changing [`enabled`][1] property of
    /// [MediaStreamTrack][2].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams#dom-mediastreamtrack-enabled
    /// [2]: https://w3.org/TR/mediacapture-streams#dom-mediastreamtrack
    Mute(mute_state::Stable),

    /// Responsible for changing [RTCRtpTransceiverDirection][1] to stop
    /// traffic flow.
    ///
    /// Requires renegotiation for changes to take an effect.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiverdirection
    MediaExchange(media_exchange_state::Stable),
}

impl MediaState {
    /// Generates [`TrackPatchCommand`] with a provided [`TrackId`] basing on
    /// this [`MediaState`].
    ///
    /// If [`MediaState`] is [`MediaState::Mute`] then
    /// [`TrackPatchCommand::muted`] will be [`Some`].
    ///
    /// If [`MediaState`] is [`MediaState::MediaExchange`] then
    /// [`TrackPatchCommand::enabled`] will be [`Some`].
    #[must_use]
    pub fn generate_track_patch(self, track_id: TrackId) -> TrackPatchCommand {
        match self {
            Self::Mute(mute) => TrackPatchCommand {
                id: track_id,
                muted: Some(mute == mute_state::Stable::Muted),
                enabled: None,
            },
            Self::MediaExchange(media_exchange) => TrackPatchCommand {
                id: track_id,
                enabled: Some(
                    media_exchange == media_exchange_state::Stable::Enabled,
                ),
                muted: None,
            },
        }
    }

    /// Returns the opposite value to this [`mute_state::Stable`].
    #[inline]
    #[must_use]
    pub fn opposite(self) -> Self {
        match self {
            Self::Mute(mute) => Self::Mute(mute.opposite()),
            Self::MediaExchange(media_exchange) => {
                Self::MediaExchange(media_exchange.opposite())
            }
        }
    }
}

/// [`TransitableState::Stable`] variant of the [`TransitableState`].
pub trait InStable: Clone + Copy + PartialEq {
    /// Transition invariants of this [`InStable`].
    type Transition: InTransition;

    /// Converts this [`InStable`] into [`InStable::Transition`].
    #[must_use]
    fn start_transition(self) -> Self::Transition;
}

/// [`TransitableState::Transition`] variant of the [`TransitableState`].
pub trait InTransition: Clone + Copy + PartialEq {
    /// Stable invariants of this [`InTransition`].
    type Stable: InStable;

    /// Returns intention which this state indicates.
    #[must_use]
    fn intended(self) -> Self::Stable;

    /// Sets inner [`InTransition::Stable`] state.
    #[must_use]
    fn set_inner(self, inner: Self::Stable) -> Self;

    /// Returns inner [`InTransition::Stable`] state.
    #[must_use]
    fn into_inner(self) -> Self::Stable;

    /// Returns opposite to this [`InTransition`].
    #[must_use]
    fn opposite(self) -> Self;
}

/// All media exchange states in which [`MediaStateControllable`] can be.
///
/// [`MediaStateControllable`]: crate::peer::MediaStateControllable
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitableState<S, T> {
    /// State of transition.
    Transition(T),

    /// Stable state.
    Stable(S),
}

impl<S, T> TransitableState<S, T>
where
    T: InTransition<Stable = S> + Into<TransitableState<S, T>>,
    S: InStable<Transition = T> + Into<TransitableState<S, T>>,
{
    /// Starts transition into the `desired_state` changing the state to
    /// [`TransitableState::Transition`].
    ///
    /// No-op if already in the `desired_state`.
    #[must_use]
    pub fn transition_to(self, desired_state: S) -> Self {
        if self == desired_state.into() {
            return self;
        }
        match self {
            Self::Stable(stable) => stable.start_transition().into(),
            Self::Transition(transition) => {
                if transition.intended() == desired_state {
                    self
                } else {
                    transition.opposite().into()
                }
            }
        }
    }

    /// Cancels an ongoing transition, if any.
    #[inline]
    #[must_use]
    pub fn cancel_transition(self) -> Self {
        match self {
            Self::Stable(_) => self,
            Self::Transition(t) => t.into_inner().into(),
        }
    }
}

impl From<media_exchange_state::Stable> for MediaExchangeState {
    #[inline]
    fn from(from: media_exchange_state::Stable) -> Self {
        Self::Stable(from)
    }
}

impl From<media_exchange_state::Transition> for MediaExchangeState {
    #[inline]
    fn from(from: media_exchange_state::Transition) -> Self {
        Self::Transition(from)
    }
}

impl From<mute_state::Stable> for MuteState {
    #[inline]
    fn from(from: mute_state::Stable) -> Self {
        Self::Stable(from)
    }
}

impl From<mute_state::Transition> for MuteState {
    #[inline]
    fn from(from: mute_state::Transition) -> Self {
        Self::Transition(from)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const DISABLED: MediaExchangeState =
        TransitableState::Stable(media_exchange_state::Stable::Disabled);
    const ENABLED: MediaExchangeState =
        TransitableState::Stable(media_exchange_state::Stable::Enabled);
    const ENABLING_DISABLED: MediaExchangeState = TransitableState::Transition(
        media_exchange_state::Transition::Enabling(
            media_exchange_state::Stable::Disabled,
        ),
    );
    const ENABLING_ENABLED: MediaExchangeState = TransitableState::Transition(
        media_exchange_state::Transition::Enabling(
            media_exchange_state::Stable::Enabled,
        ),
    );
    const DISABLING_DISABLED: MediaExchangeState = TransitableState::Transition(
        media_exchange_state::Transition::Disabling(
            media_exchange_state::Stable::Disabled,
        ),
    );
    const DISABLING_ENABLED: MediaExchangeState = TransitableState::Transition(
        media_exchange_state::Transition::Disabling(
            media_exchange_state::Stable::Enabled,
        ),
    );

    #[test]
    fn transition_to() {
        assert_eq!(
            DISABLED.transition_to(media_exchange_state::Stable::Disabled),
            DISABLED
        );
        assert_eq!(
            DISABLED.transition_to(media_exchange_state::Stable::Enabled),
            ENABLING_DISABLED
        );
        assert_eq!(
            ENABLED.transition_to(media_exchange_state::Stable::Enabled),
            ENABLED
        );
        assert_eq!(
            ENABLED.transition_to(media_exchange_state::Stable::Disabled),
            DISABLING_ENABLED
        );

        assert_eq!(
            ENABLING_DISABLED
                .transition_to(media_exchange_state::Stable::Disabled),
            DISABLING_DISABLED
        );
        assert_eq!(
            ENABLING_DISABLED
                .transition_to(media_exchange_state::Stable::Enabled),
            ENABLING_DISABLED
        );
        assert_eq!(
            DISABLING_ENABLED
                .transition_to(media_exchange_state::Stable::Disabled),
            DISABLING_ENABLED
        );
        assert_eq!(
            DISABLING_ENABLED
                .transition_to(media_exchange_state::Stable::Enabled),
            ENABLING_ENABLED
        );
        assert_eq!(
            DISABLING_DISABLED
                .transition_to(media_exchange_state::Stable::Disabled),
            DISABLING_DISABLED
        );
        assert_eq!(
            DISABLING_DISABLED
                .transition_to(media_exchange_state::Stable::Enabled),
            ENABLING_DISABLED
        );
        assert_eq!(
            ENABLING_ENABLED
                .transition_to(media_exchange_state::Stable::Disabled),
            DISABLING_ENABLED
        );
        assert_eq!(
            ENABLING_ENABLED
                .transition_to(media_exchange_state::Stable::Enabled),
            ENABLING_ENABLED
        );
    }

    #[test]
    fn cancel_transition() {
        assert_eq!(DISABLED.cancel_transition(), DISABLED);
        assert_eq!(ENABLED.cancel_transition(), ENABLED);
        assert_eq!(ENABLING_DISABLED.cancel_transition(), DISABLED);
        assert_eq!(ENABLING_ENABLED.cancel_transition(), ENABLED);
        assert_eq!(DISABLING_DISABLED.cancel_transition(), DISABLED);
        assert_eq!(DISABLING_ENABLED.cancel_transition(), ENABLED);
    }
}
