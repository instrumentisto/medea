//! [`PeerConnection`]s [`Sender`] publish state.
//!
//! [`PeerConnection`]: crate::peer::PeerConnection

use derive_more::From;

/// All publish states in which [`Sender`] can be.
///
/// [`Sender`]: super::Sender
#[derive(Clone, Copy, Debug, From, Eq, PartialEq)]
pub enum PublishState {
    /// State of transition.
    Transition(PublishStateTransition),

    /// Stable state.
    Stable(StablePublishState),
}

impl PublishState {
    /// Indicates whether [`PublishState`] is stable (not in transition).
    #[inline]
    pub fn is_stable(self) -> bool {
        match self {
            PublishState::Stable(_) => true,
            PublishState::Transition(_) => false,
        }
    }

    /// Starts transition into the `desired_state` changing the state to
    /// [`PublishState::Transition`].
    ///
    /// No-op if already in the `desired_state`.
    pub fn transition_to(self, desired_state: StablePublishState) -> Self {
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
                        PublishStateTransition::Enabling(from) => {
                            PublishStateTransition::Disabling(from)
                        }
                        PublishStateTransition::Disabling(from) => {
                            PublishStateTransition::Enabling(from)
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

/// Stable [`PublishState`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StablePublishState {
    /// [`Sender`] is enabled.
    ///
    /// [`Sender`]: super::Sender
    Enabled,

    /// [`Sender`] is disabled.
    ///
    /// [`Sender`]: super::Sender
    Disabled,
}

impl StablePublishState {
    /// Converts this [`StablePublishState`] into [`PublishStateTransition`].
    ///
    /// [`StablePublishState::Enabled`] => [`PublishStateTransition::Muting`].
    ///
    /// [`StablePublishState::Disabled`] =>
    /// [`PublishStateTransition::Unmuting`].
    #[inline]
    pub fn start_transition(self) -> PublishStateTransition {
        match self {
            Self::Enabled => PublishStateTransition::Disabling(self),
            Self::Disabled => PublishStateTransition::Enabling(self),
        }
    }
}

impl From<bool> for StablePublishState {
    #[inline]
    fn from(is_enabled: bool) -> Self {
        if is_enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }
}

/// [`PublishState`] in transition to another [`StablePublishState`].
///
/// [`StablePublishState`] which is stored in [`PublishStateTransition`]
/// variants is a state which we already have, but we still waiting for a
/// desired state update. If desired state update won't be received, then the
/// stored [`StablePublishState`] will be applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublishStateTransition {
    /// [`Sender`] should be enabled, but awaits server permission.
    ///
    /// [`Sender`]: super::Sender
    Enabling(StablePublishState),

    /// [`Sender`] should be disabled, but awaits server permission.
    ///
    /// [`Sender`]: super::Sender
    Disabling(StablePublishState),
}

impl PublishStateTransition {
    /// Returns intention which this [`PublishStateTransition`] indicates.
    #[inline]
    pub fn intended(self) -> StablePublishState {
        match self {
            Self::Enabling(_) => StablePublishState::Enabled,
            Self::Disabling(_) => StablePublishState::Disabled,
        }
    }

    /// Sets inner [`StablePublishState`].
    #[inline]
    pub fn set_inner(self, inner: StablePublishState) -> Self {
        match self {
            Self::Enabling(_) => Self::Enabling(inner),
            Self::Disabling(_) => Self::Disabling(inner),
        }
    }

    /// Returns inner [`StablePublishState`].
    #[inline]
    pub fn into_inner(self) -> StablePublishState {
        match self {
            Self::Enabling(s) | Self::Disabling(s) => s,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const MUTED: PublishState =
        PublishState::Stable(StablePublishState::Disabled);
    const NOT_MUTED: PublishState =
        PublishState::Stable(StablePublishState::Enabled);
    const UNMUTING_MUTED: PublishState = PublishState::Transition(
        PublishStateTransition::Enabling(StablePublishState::Disabled),
    );
    const UNMUTING_NOT_MUTED: PublishState = PublishState::Transition(
        PublishStateTransition::Enabling(StablePublishState::Enabled),
    );
    const MUTING_MUTED: PublishState = PublishState::Transition(
        PublishStateTransition::Disabling(StablePublishState::Disabled),
    );
    const MUTING_NOT_MUTED: PublishState = PublishState::Transition(
        PublishStateTransition::Disabling(StablePublishState::Enabled),
    );

    #[test]
    fn transition_to() {
        assert_eq!(MUTED.transition_to(StablePublishState::Disabled), MUTED);
        assert_eq!(
            MUTED.transition_to(StablePublishState::Enabled),
            UNMUTING_MUTED
        );
        assert_eq!(
            NOT_MUTED.transition_to(StablePublishState::Enabled),
            NOT_MUTED
        );
        assert_eq!(
            NOT_MUTED.transition_to(StablePublishState::Disabled),
            MUTING_NOT_MUTED
        );

        assert_eq!(
            UNMUTING_MUTED.transition_to(StablePublishState::Disabled),
            MUTING_MUTED
        );
        assert_eq!(
            UNMUTING_MUTED.transition_to(StablePublishState::Enabled),
            UNMUTING_MUTED
        );
        assert_eq!(
            MUTING_NOT_MUTED.transition_to(StablePublishState::Disabled),
            MUTING_NOT_MUTED
        );
        assert_eq!(
            MUTING_NOT_MUTED.transition_to(StablePublishState::Enabled),
            UNMUTING_NOT_MUTED
        );
        assert_eq!(
            MUTING_MUTED.transition_to(StablePublishState::Disabled),
            MUTING_MUTED
        );
        assert_eq!(
            MUTING_MUTED.transition_to(StablePublishState::Enabled),
            UNMUTING_MUTED
        );
        assert_eq!(
            UNMUTING_NOT_MUTED.transition_to(StablePublishState::Disabled),
            MUTING_NOT_MUTED
        );
        assert_eq!(
            UNMUTING_NOT_MUTED.transition_to(StablePublishState::Enabled),
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
