//! [`Disableable`]s media exchange state.
//!
//! [`Disableable`]: super::Disableable

mod controller;

use derive_more::From;

pub use self::controller::Controller;

/// All media exchange states in which [`Disableable`] can be.
///
/// [`Disableable`]: super::Disableable
#[derive(Clone, Copy, Debug, From, Eq, PartialEq)]
pub enum State {
    /// State of transition.
    Transition(Transition),

    /// Stable state.
    Stable(Stable),
}

impl State {
    /// Indicates whether [`State`] is stable (not in transition).
    #[inline]
    #[must_use]
    pub fn is_stable(self) -> bool {
        match self {
            State::Stable(_) => true,
            State::Transition(_) => false,
        }
    }

    /// Starts transition into the `desired_state` changing the state to
    /// [`State::Transition`].
    ///
    /// No-op if already in the `desired_state`.
    #[must_use]
    pub fn transition_to(self, desired_state: Stable) -> Self {
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
                        Transition::Enabling(from) => {
                            Transition::Disabling(from)
                        }
                        Transition::Disabling(from) => {
                            Transition::Enabling(from)
                        }
                    }
                    .into()
                }
            }
        }
    }

    /// Cancels ongoing transition if any.
    #[inline]
    #[must_use]
    pub fn cancel_transition(self) -> Self {
        match self {
            Self::Stable(_) => self,
            Self::Transition(t) => t.into_inner().into(),
        }
    }
}

/// Stable [`State`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stable {
    /// [`Disableable`] is enabled.
    ///
    /// [`Disableable`]: super::Disableable
    Enabled,

    /// [`Disableable`] is disabled.
    ///
    /// [`Disableable`]: super::Disableable
    Disabled,
}

impl Stable {
    /// Converts this [`Stable`] into [`Transition`].
    ///
    /// [`Stable::Enabled`] => [`Transition::Disabling`].
    ///
    /// [`Stable::Disabled`] => [`Transition::Enabling`].
    #[inline]
    #[must_use]
    pub fn start_transition(self) -> Transition {
        match self {
            Self::Enabled => Transition::Disabling(self),
            Self::Disabled => Transition::Enabling(self),
        }
    }
}

impl From<bool> for Stable {
    #[inline]
    fn from(enabled: bool) -> Self {
        if enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }
}

/// [`State`] in transition to another [`Stable`].
///
/// [`Stable`] which is stored in [`Transition`] variants is a state which we
/// already have, but we still waiting for a desired state update. If desired
/// state update won't be received, then the stored [`Stable`] will be applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transition {
    /// [`Disableable`] should be enabled, but awaits server permission.
    ///
    /// [`Disableable`]: super::Disableable
    Enabling(Stable),

    /// [`Disableable`] should be disabled, but awaits server permission.
    ///
    /// [`Disableable`]: super::Disableable
    Disabling(Stable),
}

impl Transition {
    /// Returns intention which this [`Transition`] indicates.
    #[inline]
    #[must_use]
    pub fn intended(self) -> Stable {
        match self {
            Self::Enabling(_) => Stable::Enabled,
            Self::Disabling(_) => Stable::Disabled,
        }
    }

    /// Sets inner [`Stable`].
    #[inline]
    #[must_use]
    pub fn set_inner(self, inner: Stable) -> Self {
        match self {
            Self::Enabling(_) => Self::Enabling(inner),
            Self::Disabling(_) => Self::Disabling(inner),
        }
    }

    /// Returns inner [`Stable`].
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> Stable {
        match self {
            Self::Enabling(s) | Self::Disabling(s) => s,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const DISABLED: State = State::Stable(Stable::Disabled);
    const ENABLED: State = State::Stable(Stable::Enabled);
    const ENABLING_DISABLED: State =
        State::Transition(Transition::Enabling(Stable::Disabled));
    const ENABLING_ENABLED: State =
        State::Transition(Transition::Enabling(Stable::Enabled));
    const DISABLING_DISABLED: State =
        State::Transition(Transition::Disabling(Stable::Disabled));
    const DISABLING_ENABLED: State =
        State::Transition(Transition::Disabling(Stable::Enabled));

    #[test]
    fn transition_to() {
        assert_eq!(DISABLED.transition_to(Stable::Disabled), DISABLED);
        assert_eq!(DISABLED.transition_to(Stable::Enabled), ENABLING_DISABLED);
        assert_eq!(ENABLED.transition_to(Stable::Enabled), ENABLED);
        assert_eq!(ENABLED.transition_to(Stable::Disabled), DISABLING_ENABLED);

        assert_eq!(
            ENABLING_DISABLED.transition_to(Stable::Disabled),
            DISABLING_DISABLED
        );
        assert_eq!(
            ENABLING_DISABLED.transition_to(Stable::Enabled),
            ENABLING_DISABLED
        );
        assert_eq!(
            DISABLING_ENABLED.transition_to(Stable::Disabled),
            DISABLING_ENABLED
        );
        assert_eq!(
            DISABLING_ENABLED.transition_to(Stable::Enabled),
            ENABLING_ENABLED
        );
        assert_eq!(
            DISABLING_DISABLED.transition_to(Stable::Disabled),
            DISABLING_DISABLED
        );
        assert_eq!(
            DISABLING_DISABLED.transition_to(Stable::Enabled),
            ENABLING_DISABLED
        );
        assert_eq!(
            ENABLING_ENABLED.transition_to(Stable::Disabled),
            DISABLING_ENABLED
        );
        assert_eq!(
            ENABLING_ENABLED.transition_to(Stable::Enabled),
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
