//! [`Disableable`]s media exchange state.
//!
//! [`Disableable`]: super::Disableable

mod controller;

use derive_more::From;

pub use self::controller::MediaExchangeStateController;

/// All media exchange states in which [`Disableable`] can be.
///
/// [`Disableable`]: super::Disableable
#[derive(Clone, Copy, Debug, From, Eq, PartialEq)]
pub enum MediaExchangeState {
    /// State of transition.
    Transition(MediaExchangeStateTransition),

    /// Stable state.
    Stable(StableMediaExchangeState),
}

impl MediaExchangeState {
    /// Indicates whether [`MediaExchangeState`] is stable (not in transition).
    #[inline]
    pub fn is_stable(self) -> bool {
        match self {
            MediaExchangeState::Stable(_) => true,
            MediaExchangeState::Transition(_) => false,
        }
    }

    /// Starts transition into the `desired_state` changing the state to
    /// [`MediaExchangeState::Transition`].
    ///
    /// No-op if already in the `desired_state`.
    pub fn transition_to(
        self,
        desired_state: StableMediaExchangeState,
    ) -> Self {
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
                        MediaExchangeStateTransition::Enabling(from) => {
                            MediaExchangeStateTransition::Disabling(from)
                        }
                        MediaExchangeStateTransition::Disabling(from) => {
                            MediaExchangeStateTransition::Enabling(from)
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

/// Stable [`MediaExchangeState`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableMediaExchangeState {
    /// [`Disableable`] is enabled.
    ///
    /// [`Disableable`]: super::Disableable
    Enabled,

    /// [`Disableable`] is disabled.
    ///
    /// [`Disableable`]: super::Disableable
    Disabled,
}

impl StableMediaExchangeState {
    /// Converts this [`StableMediaExchangeState`] into
    /// [`MediaExchangeStateTransition`].
    ///
    /// [`StableMediaExchangeState::Enabled`] =>
    /// [`MediaExchangeStateTransition::Disabling`].
    ///
    /// [`StableMediaExchangeState::Disabled`] =>
    /// [`MediaExchangeStateTransition::Enabling`].
    #[inline]
    pub fn start_transition(self) -> MediaExchangeStateTransition {
        match self {
            Self::Enabled => MediaExchangeStateTransition::Disabling(self),
            Self::Disabled => MediaExchangeStateTransition::Enabling(self),
        }
    }
}

impl From<bool> for StableMediaExchangeState {
    #[inline]
    fn from(is_disabled: bool) -> Self {
        if is_disabled {
            Self::Disabled
        } else {
            Self::Enabled
        }
    }
}

/// [`MediaExchangeState`] in transition to another
/// [`StableMediaExchangeState`].
///
/// [`StableMediaExchangeState`] which is stored in
/// [`MediaExchangeStateTransition`] variants is a state which we already have,
/// but we still waiting for a desired state update. If desired state update
/// won't be received, then the stored [`StableMediaExchangeState`] will be
/// applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaExchangeStateTransition {
    /// [`Disableable`] should be enabled, but awaits server permission.
    ///
    /// [`Disableable`]: super::Disableable
    Enabling(StableMediaExchangeState),

    /// [`Disableable`] should be disabled, but awaits server permission.
    ///
    /// [`Disableable`]: super::Disableable
    Disabling(StableMediaExchangeState),
}

impl MediaExchangeStateTransition {
    /// Returns intention which this [`MediaExchangeStateTransition`] indicates.
    #[inline]
    pub fn intended(self) -> StableMediaExchangeState {
        match self {
            Self::Enabling(_) => StableMediaExchangeState::Enabled,
            Self::Disabling(_) => StableMediaExchangeState::Disabled,
        }
    }

    /// Sets inner [`StableMediaExchangeState`].
    #[inline]
    pub fn set_inner(self, inner: StableMediaExchangeState) -> Self {
        match self {
            Self::Enabling(_) => Self::Enabling(inner),
            Self::Disabling(_) => Self::Disabling(inner),
        }
    }

    /// Returns inner [`StableMediaExchangeState`].
    #[inline]
    pub fn into_inner(self) -> StableMediaExchangeState {
        match self {
            Self::Enabling(s) | Self::Disabling(s) => s,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const DISABLED: MediaExchangeState =
        MediaExchangeState::Stable(StableMediaExchangeState::Disabled);
    const ENABLED: MediaExchangeState =
        MediaExchangeState::Stable(StableMediaExchangeState::Enabled);
    const ENABLING_DISABLED: MediaExchangeState =
        MediaExchangeState::Transition(MediaExchangeStateTransition::Enabling(
            StableMediaExchangeState::Disabled,
        ));
    const ENABLING_ENABLED: MediaExchangeState =
        MediaExchangeState::Transition(MediaExchangeStateTransition::Enabling(
            StableMediaExchangeState::Enabled,
        ));
    const DISABLING_DISABLED: MediaExchangeState =
        MediaExchangeState::Transition(
            MediaExchangeStateTransition::Disabling(
                StableMediaExchangeState::Disabled,
            ),
        );
    const DISABLING_ENABLED: MediaExchangeState =
        MediaExchangeState::Transition(
            MediaExchangeStateTransition::Disabling(
                StableMediaExchangeState::Enabled,
            ),
        );

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
