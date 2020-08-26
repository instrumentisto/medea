//! [`PeerConnection`]s [`MuteableTrack`] mute state.
//!
//! [`PeerConnection`]: crate::peer::PeerConnection

mod controller;

use derive_more::From;

pub use self::controller::MuteStateController;

/// All mute states in which [`MuteableTrack`] can be.
///
/// [`MuteableTrack`]: super::MuteableTrack
#[derive(Clone, Copy, Debug, From, Eq, PartialEq)]
pub enum MuteState {
    /// State of transition.
    Transition(MuteStateTransition),

    /// Stable state.
    Stable(StableMuteState),
}

impl MuteState {
    /// Indicates whether [`MuteState`] is stable (not in transition).
    #[inline]
    pub fn is_stable(self) -> bool {
        match self {
            MuteState::Stable(_) => true,
            MuteState::Transition(_) => false,
        }
    }

    /// Starts transition into the `desired_state` changing the state to
    /// [`MuteState::Transition`].
    ///
    /// No-op if already in the `desired_state`.
    pub fn transition_to(self, desired_state: StableMuteState) -> Self {
        if self == desired_state.into() {
            return self;
        }
        match self {
            Self::Stable(stable) => stable.start_transition().into(),
            Self::Transition(transition) => {
                if transition.intended() == desired_state {
                    self
                } else {
                    match transition {
                        MuteStateTransition::Unmuting(from) => {
                            MuteStateTransition::Muting(from)
                        }
                        MuteStateTransition::Muting(from) => {
                            MuteStateTransition::Unmuting(from)
                        }
                    }
                    .into()
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

/// Stable [`MuteState`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableMuteState {
    /// [`MuteableTrack`] is not muted.
    ///
    /// [`MuteableTrack`]: super::MuteableTrack
    NotMuted,

    /// [`MuteableTrack`] is muted.
    ///
    /// [`MuteableTrack`]: super::MuteableTrack
    Muted,
}

impl StableMuteState {
    /// Converts this [`StableMuteState`] into [`MuteStateTransition`].
    ///
    /// [`StableMuteState::NotMuted`] => [`MuteStateTransition::Muting`].
    ///
    /// [`StableMuteState::Muted`] => [`MuteStateTransition::Unmuting`].
    #[inline]
    pub fn start_transition(self) -> MuteStateTransition {
        match self {
            Self::NotMuted => MuteStateTransition::Muting(self),
            Self::Muted => MuteStateTransition::Unmuting(self),
        }
    }
}

impl From<bool> for StableMuteState {
    #[inline]
    fn from(is_muted: bool) -> Self {
        if is_muted {
            Self::Muted
        } else {
            Self::NotMuted
        }
    }
}

/// [`MuteState`] in transition to another [`StableMuteState`].
///
/// [`StableMuteState`] which is stored in [`MuteStateTransition`] variants
/// is a state which we already have, but we still waiting for a desired state
/// update. If desired state update won't be received, then the stored
/// [`StableMuteState`] will be applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MuteStateTransition {
    /// [`MuteableTrack`] should be unmuted, but awaits server permission.
    ///
    /// [`MuteableTrack`]: super::MuteableTrack
    Unmuting(StableMuteState),

    /// [`MuteableTrack`] should be muted, but awaits server permission.
    ///
    /// [`MuteableTrack`]: super::MuteableTrack
    Muting(StableMuteState),
}

impl MuteStateTransition {
    /// Returns intention which this [`MuteStateTransition`] indicates.
    #[inline]
    pub fn intended(self) -> StableMuteState {
        match self {
            Self::Unmuting(_) => StableMuteState::NotMuted,
            Self::Muting(_) => StableMuteState::Muted,
        }
    }

    /// Sets inner [`StableMuteState`].
    #[inline]
    pub fn set_inner(self, inner: StableMuteState) -> Self {
        match self {
            Self::Unmuting(_) => Self::Unmuting(inner),
            Self::Muting(_) => Self::Muting(inner),
        }
    }

    /// Returns inner [`StableMuteState`].
    #[inline]
    pub fn into_inner(self) -> StableMuteState {
        match self {
            Self::Unmuting(s) | Self::Muting(s) => s,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const MUTED: MuteState = MuteState::Stable(StableMuteState::Muted);
    const NOT_MUTED: MuteState = MuteState::Stable(StableMuteState::NotMuted);
    const UNMUTING_MUTED: MuteState = MuteState::Transition(
        MuteStateTransition::Unmuting(StableMuteState::Muted),
    );
    const UNMUTING_NOT_MUTED: MuteState = MuteState::Transition(
        MuteStateTransition::Unmuting(StableMuteState::NotMuted),
    );
    const MUTING_MUTED: MuteState = MuteState::Transition(
        MuteStateTransition::Muting(StableMuteState::Muted),
    );
    const MUTING_NOT_MUTED: MuteState = MuteState::Transition(
        MuteStateTransition::Muting(StableMuteState::NotMuted),
    );

    #[test]
    fn transition_to() {
        assert_eq!(MUTED.transition_to(StableMuteState::Muted), MUTED);
        assert_eq!(
            MUTED.transition_to(StableMuteState::NotMuted),
            UNMUTING_MUTED
        );
        assert_eq!(
            NOT_MUTED.transition_to(StableMuteState::NotMuted),
            NOT_MUTED
        );
        assert_eq!(
            NOT_MUTED.transition_to(StableMuteState::Muted),
            MUTING_NOT_MUTED
        );

        assert_eq!(
            UNMUTING_MUTED.transition_to(StableMuteState::Muted),
            MUTING_MUTED
        );
        assert_eq!(
            UNMUTING_MUTED.transition_to(StableMuteState::NotMuted),
            UNMUTING_MUTED
        );
        assert_eq!(
            MUTING_NOT_MUTED.transition_to(StableMuteState::Muted),
            MUTING_NOT_MUTED
        );
        assert_eq!(
            MUTING_NOT_MUTED.transition_to(StableMuteState::NotMuted),
            UNMUTING_NOT_MUTED
        );
        assert_eq!(
            MUTING_MUTED.transition_to(StableMuteState::Muted),
            MUTING_MUTED
        );
        assert_eq!(
            MUTING_MUTED.transition_to(StableMuteState::NotMuted),
            UNMUTING_MUTED
        );
        assert_eq!(
            UNMUTING_NOT_MUTED.transition_to(StableMuteState::Muted),
            MUTING_NOT_MUTED
        );
        assert_eq!(
            UNMUTING_NOT_MUTED.transition_to(StableMuteState::NotMuted),
            UNMUTING_NOT_MUTED
        );
    }

    #[test]
    fn cancel_transition() {
        assert_eq!(MUTED.cancel_transition(), MUTED);
        assert_eq!(NOT_MUTED.cancel_transition(), NOT_MUTED);
        assert_eq!(UNMUTING_MUTED.cancel_transition(), MUTED);
        assert_eq!(UNMUTING_NOT_MUTED.cancel_transition(), NOT_MUTED);
        assert_eq!(MUTING_MUTED.cancel_transition(), MUTED);
        assert_eq!(MUTING_NOT_MUTED.cancel_transition(), NOT_MUTED);
    }
}
