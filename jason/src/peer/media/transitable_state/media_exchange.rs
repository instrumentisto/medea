use super::{InStable, InTransition};

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
    pub fn inverse(self) -> Self {
        match self {
            Self::Enabled => Self::Disabled,
            Self::Disabled => Self::Enabled
        }
    }
}

impl InStable for StableMediaExchangeState {
    type Transition = TransitionMediaExchangeState;

    /// Converts this [`StableMediaExchangeState`] into
    /// [`MediaExchangeStateTransition`].
    ///
    /// [`StableMediaExchangeState::Enabled`] =>
    /// [`MediaExchangeStateTransition::Disabling`].
    ///
    /// [`StableMediaExchangeState::Disabled`] =>
    /// [`MediaExchangeStateTransition::Enabling`].
    #[inline]
    fn start_transition(self) -> Self::Transition {
        match self {
            Self::Enabled => TransitionMediaExchangeState::Disabling(self),
            Self::Disabled => TransitionMediaExchangeState::Enabling(self),
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
pub enum TransitionMediaExchangeState {
    /// [`Disableable`] should be enabled, but awaits server permission.
    ///
    /// [`Disableable`]: super::Disableable
    Enabling(StableMediaExchangeState),

    /// [`Disableable`] should be disabled, but awaits server permission.
    ///
    /// [`Disableable`]: super::Disableable
    Disabling(StableMediaExchangeState),
}

impl InTransition for TransitionMediaExchangeState {
    type Stable = StableMediaExchangeState;

    /// Returns intention which this [`MediaExchangeStateTransition`] indicates.
    #[inline]
    fn intended(self) -> Self::Stable {
        match self {
            Self::Enabling(_) => StableMediaExchangeState::Enabled,
            Self::Disabling(_) => StableMediaExchangeState::Disabled,
        }
    }

    /// Sets inner [`StableMediaExchangeState`].
    #[inline]
    fn set_inner(self, inner: Self::Stable) -> Self {
        match self {
            Self::Enabling(_) => Self::Enabling(inner),
            Self::Disabling(_) => Self::Disabling(inner),
        }
    }

    /// Returns inner [`StableMediaExchangeState`].
    #[inline]
    fn into_inner(self) -> Self::Stable {
        match self {
            Self::Enabling(s) | Self::Disabling(s) => s,
        }
    }

    #[inline]
    fn reverse(self) -> Self {
        match self {
            Self::Enabling(stable) => Self::Disabling(stable),
            Self::Disabling(stable) => Self::Enabling(stable),
        }
    }
}

impl TransitionMediaExchangeState {
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
