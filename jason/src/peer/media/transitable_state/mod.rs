//! [`MediaStateControllable`]s media exchange state.
//!
//! [`MediaStateControllable`]: super::MediaStateControllable

mod controller;
mod media_exchange;
mod mute;

use derive_more::From;
use medea_client_api_proto::{TrackId, TrackPatchCommand};

pub use self::{
    controller::{
        MediaExchangeStateController, MuteStateController,
        TransitableStateController,
    },
    media_exchange::{StableMediaExchangeState, TransitionMediaExchangeState},
    mute::{StableMuteState, TransitionMuteState},
};

/// [`TransitableState`] for the [`StableMediaExchangeState`].
pub type MediaExchangeState =
    TransitableState<StableMediaExchangeState, TransitionMediaExchangeState>;
/// [`TransitableState`] for the [`StableMuteState`].
pub type MuteState = TransitableState<StableMuteState, TransitionMuteState>;

/// All media states which can be toggled in the [`MediaStateControllable`].
#[derive(Clone, Copy, Debug, From)]
pub enum MediaState {
    /// Sets `MediaStreamTrack.enabled` to the `true` of `false`.
    ///
    /// Doesn't requires renegotiation process, but traffic flow doesn't stops.
    Mute(StableMuteState),

    /// Drops `MediaStreamTrack` if [`StableMediaExchangeState::Disabled`].
    ///
    /// Requires renegotiation process and traffic flow will be stopped.
    MediaExchange(StableMediaExchangeState),
}

impl MediaState {
    /// Generates [`TrackPatchCommand`] with a provided [`TrackId`] based on
    /// this [`MediaState`].
    ///
    /// If [`MediaState`] is [`MediaState::Mute`] then
    /// [`TrackPatchCommand::is_muted`] will be [`Some`].
    ///
    /// If [`MediaState`] is [`MediaState::MediaExchange`] then
    /// [`TrackPatchCommand::is_disabled`] will be [`Some`].
    pub fn generate_track_patch(self, track_id: TrackId) -> TrackPatchCommand {
        match self {
            Self::Mute(mute) => TrackPatchCommand {
                id: track_id,
                is_muted: Some(mute == StableMuteState::Muted),
                is_disabled: None,
            },
            Self::MediaExchange(media_exchange) => TrackPatchCommand {
                id: track_id,
                is_disabled: Some(
                    media_exchange == StableMediaExchangeState::Disabled,
                ),
                is_muted: None,
            },
        }
    }

    /// Returns opposite to this [`StableMuteState`].
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
    type Transition: InTransition;

    /// Converts this [`InStable`] into [`InStable::Transition`].
    fn start_transition(self) -> Self::Transition;
}

/// [`TransitableState::Transition`] variant of the [`TransitableState`].
pub trait InTransition: Clone + Copy + PartialEq {
    type Stable: InStable;

    /// Returns intention which this state indicates.
    fn intended(self) -> Self::Stable;

    /// Sets inner [`InTransition::Stable`] state.
    fn set_inner(self, inner: Self::Stable) -> Self;

    /// Returns inner [`InTransition::Stable`] state.
    fn into_inner(self) -> Self::Stable;

    /// Returns opposite to this [`InTransition`].
    fn opposite(self) -> Self;
}

/// All media exchange states in which [`MediaStateControllable`] can be.
///
/// [`MediaStateControllable`]: super::MediaStateControllable
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
    /// Indicates whether [`TransitableState`] is stable (not in transition).
    #[inline]
    pub fn is_stable(self) -> bool {
        match self {
            TransitableState::Stable(_) => true,
            TransitableState::Transition(_) => false,
        }
    }

    /// Starts transition into the `desired_state` changing the state to
    /// [`TransitableState::Transition`].
    ///
    /// No-op if already in the `desired_state`.
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

    /// Cancels ongoing transition if any.
    #[inline]
    pub fn cancel_transition(self) -> Self {
        match self {
            Self::Stable(_) => self,
            Self::Transition(t) => t.into_inner().into(),
        }
    }
}

impl From<StableMediaExchangeState> for MediaExchangeState {
    fn from(from: StableMediaExchangeState) -> Self {
        Self::Stable(from)
    }
}

impl From<TransitionMediaExchangeState> for MediaExchangeState {
    fn from(from: TransitionMediaExchangeState) -> Self {
        Self::Transition(from)
    }
}

impl From<StableMuteState> for MuteState {
    fn from(from: StableMuteState) -> Self {
        Self::Stable(from)
    }
}

impl From<TransitionMuteState> for MuteState {
    fn from(from: TransitionMuteState) -> Self {
        Self::Transition(from)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const DISABLED: MediaExchangeState =
        TransitableState::Stable(StableMediaExchangeState::Disabled);
    const ENABLED: MediaExchangeState =
        TransitableState::Stable(StableMediaExchangeState::Enabled);
    const ENABLING_DISABLED: MediaExchangeState =
        TransitableState::Transition(TransitionMediaExchangeState::Enabling(
            StableMediaExchangeState::Disabled,
        ));
    const ENABLING_ENABLED: MediaExchangeState =
        TransitableState::Transition(TransitionMediaExchangeState::Enabling(
            StableMediaExchangeState::Enabled,
        ));
    const DISABLING_DISABLED: MediaExchangeState =
        TransitableState::Transition(TransitionMediaExchangeState::Disabling(
            StableMediaExchangeState::Disabled,
        ));
    const DISABLING_ENABLED: MediaExchangeState =
        TransitableState::Transition(TransitionMediaExchangeState::Disabling(
            StableMediaExchangeState::Enabled,
        ));

    #[test]
    fn transition_to() {
        assert_eq!(
            DISABLED.transition_to(StableMediaExchangeState::Disabled),
            DISABLED
        );
        assert_eq!(
            DISABLED.transition_to(StableMediaExchangeState::Enabled),
            ENABLING_DISABLED
        );
        assert_eq!(
            ENABLED.transition_to(StableMediaExchangeState::Enabled),
            ENABLED
        );
        assert_eq!(
            ENABLED.transition_to(StableMediaExchangeState::Disabled),
            DISABLING_ENABLED
        );

        assert_eq!(
            ENABLING_DISABLED.transition_to(StableMediaExchangeState::Disabled),
            DISABLING_DISABLED
        );
        assert_eq!(
            ENABLING_DISABLED.transition_to(StableMediaExchangeState::Enabled),
            ENABLING_DISABLED
        );
        assert_eq!(
            DISABLING_ENABLED.transition_to(StableMediaExchangeState::Disabled),
            DISABLING_ENABLED
        );
        assert_eq!(
            DISABLING_ENABLED.transition_to(StableMediaExchangeState::Enabled),
            ENABLING_ENABLED
        );
        assert_eq!(
            DISABLING_DISABLED
                .transition_to(StableMediaExchangeState::Disabled),
            DISABLING_DISABLED
        );
        assert_eq!(
            DISABLING_DISABLED.transition_to(StableMediaExchangeState::Enabled),
            ENABLING_DISABLED
        );
        assert_eq!(
            ENABLING_ENABLED.transition_to(StableMediaExchangeState::Disabled),
            DISABLING_ENABLED
        );
        assert_eq!(
            ENABLING_ENABLED.transition_to(StableMediaExchangeState::Enabled),
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
