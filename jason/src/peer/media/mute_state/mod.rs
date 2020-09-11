//! [`Muteable`]s mute state.
//!
//! [`Muteable`]: super::Muteable

mod controller;

use derive_more::From;

pub use self::controller::MuteStateController;

/// All mute states in which [`Muteable`] can be.
///
/// [`Muteable`]: super::Muteable
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
    /// [`Muteable`] is not muted.
    ///
    /// [`Muteable`]: super::Muteable
    Unmuted,

    /// [`Muteable`] is muted.
    ///
    /// [`Muteable`]: super::Muteable
    Muted,
}

impl StableMuteState {
    /// Converts this [`StableMuteState`] into [`MuteStateTransition`].
    ///
    /// [`StableMuteState::Unmuted`] => [`MuteStateTransition::Muting`].
    ///
    /// [`StableMuteState::Muted`] => [`MuteStateTransition::Unmuting`].
    #[inline]
    pub fn start_transition(self) -> MuteStateTransition {
        match self {
            Self::Unmuted => MuteStateTransition::Muting(self),
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
            Self::Unmuted
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
    /// [`Muteable`] should be unmuted, but awaits server permission.
    ///
    /// [`Muteable`]: super::Muteable
    Unmuting(StableMuteState),

    /// [`Muteable`] should be muted, but awaits server permission.
    ///
    /// [`Muteable`]: super::Muteable
    Muting(StableMuteState),
}

impl MuteStateTransition {
    /// Returns intention which this [`MuteStateTransition`] indicates.
    #[inline]
    pub fn intended(self) -> StableMuteState {
        match self {
            Self::Unmuting(_) => StableMuteState::Unmuted,
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
    const NOT_MUTED: MuteState = MuteState::Stable(StableMuteState::Unmuted);
    const UNMUTING_MUTED: MuteState = MuteState::Transition(
        MuteStateTransition::Unmuting(StableMuteState::Muted),
    );
    const UNMUTING_NOT_MUTED: MuteState = MuteState::Transition(
        MuteStateTransition::Unmuting(StableMuteState::Unmuted),
    );
    const MUTING_MUTED: MuteState = MuteState::Transition(
        MuteStateTransition::Muting(StableMuteState::Muted),
    );
    const MUTING_NOT_MUTED: MuteState = MuteState::Transition(
        MuteStateTransition::Muting(StableMuteState::Unmuted),
    );

    #[test]
    fn transition_to() {
        assert_eq!(MUTED.transition_to(StableMuteState::Muted), MUTED);
        assert_eq!(
            MUTED.transition_to(StableMuteState::Unmuted),
            UNMUTING_MUTED
        );
        assert_eq!(
            NOT_MUTED.transition_to(StableMuteState::Unmuted),
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
            UNMUTING_MUTED.transition_to(StableMuteState::Unmuted),
            UNMUTING_MUTED
        );
        assert_eq!(
            MUTING_NOT_MUTED.transition_to(StableMuteState::Muted),
            MUTING_NOT_MUTED
        );
        assert_eq!(
            MUTING_NOT_MUTED.transition_to(StableMuteState::Unmuted),
            UNMUTING_NOT_MUTED
        );
        assert_eq!(
            MUTING_MUTED.transition_to(StableMuteState::Muted),
            MUTING_MUTED
        );
        assert_eq!(
            MUTING_MUTED.transition_to(StableMuteState::Unmuted),
            UNMUTING_MUTED
        );
        assert_eq!(
            UNMUTING_NOT_MUTED.transition_to(StableMuteState::Muted),
            MUTING_NOT_MUTED
        );
        assert_eq!(
            UNMUTING_NOT_MUTED.transition_to(StableMuteState::Unmuted),
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
