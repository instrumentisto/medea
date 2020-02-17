//! [`crate::peer::PeerConnection`]s [`super::Sender`] mute state.

use derive_more::From;

/// All mute states in which [`super::Sender`] can be.
#[derive(Debug, Clone, Copy, From, PartialEq)]
pub enum MuteState {
    /// Mute state in state of transition.
    Transition(MuteStateTransition),

    /// Stable mute state of [`super::Sender`].
    Stable(StableMuteState),
}

impl MuteState {
    /// Is mute state stable (not in transition state).
    pub fn is_stable(self) -> bool {
        match self {
            MuteState::Stable(_) => true,
            MuteState::Transition(_) => false,
        }
    }

    /// Starts transition into desired state changing state to
    /// [`MuteState::Transition`], no-op if already in desired state.
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
    pub fn cancel_transition(self) -> Self {
        match self {
            Self::Stable(_) => self,
            Self::Transition(transition) => transition.into_inner().into(),
        }
    }
}

/// Stable [`MuteState`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StableMuteState {
    /// [`super::Sender`] is not muted.
    NotMuted,

    /// [`super::Sender`] is muted.
    Muted,
}

impl StableMuteState {
    /// Converts this [`StableMuteState`] into [`MuteStateTransition`].
    /// [`StableMuteState::NotMuted`] => [`MuteStateTransition::Muting`].
    /// [`StableMuteState::Muted`] => [`MuteStateTransition::Unmuting`].
    pub fn start_transition(self) -> MuteStateTransition {
        match self {
            Self::NotMuted => MuteStateTransition::Muting(self),
            Self::Muted => MuteStateTransition::Unmuting(self),
        }
    }
}

impl From<bool> for StableMuteState {
    fn from(is_muted: bool) -> Self {
        if is_muted {
            Self::Muted
        } else {
            Self::NotMuted
        }
    }
}

/// [`MuteState`] in state of transition to another [`StableMuteState`] state.
///
/// [`StableMuteState`] which stored in [`MuteStateTransition`] variants
/// is state which we already have, but we still waiting for
/// needed state update. If needed a state update wouldn't be received, the
/// stored [`StableMuteState`] will be applied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MuteStateTransition {
    /// [`super::Sender`] should be unmuted, but awaits server permission.
    Unmuting(StableMuteState),

    /// [`super::Sender`] should be muted, but awaits server permission.
    Muting(StableMuteState),
}

impl MuteStateTransition {
    /// Returns intention which this [`MuteStateTransition`] indicates.
    pub fn intended(self) -> StableMuteState {
        match self {
            Self::Unmuting(_) => StableMuteState::NotMuted,
            Self::Muting(_) => StableMuteState::Muted,
        }
    }

    /// Updates inner [`StableMuteState`] state.
    pub fn set_inner(self, inner: StableMuteState) -> Self {
        match self {
            Self::Unmuting(_) => Self::Unmuting(inner),
            Self::Muting(_) => Self::Muting(inner),
        }
    }

    /// Returns inner [`StableMuteState`] state.
    pub fn into_inner(self) -> StableMuteState {
        match self {
            Self::Unmuting(available_state) | Self::Muting(available_state) => {
                available_state
            }
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
