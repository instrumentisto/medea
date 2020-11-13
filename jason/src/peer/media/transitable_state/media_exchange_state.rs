//! State of the media publishing.

use super::{InStable, InTransition};

/// State of the media publishing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stable {
    /// [`MediaStateControllable`] is enabled.
    ///
    /// [`MediaStateControllable`]: super::MediaStateControllable
    Enabled,

    /// [`MediaStateControllable`] is disabled.
    ///
    /// [`MediaStateControllable`]: super::MediaStateControllable
    Disabled,
}

impl Stable {
    /// Returns opposite to this [`StableMediaExchangeState`].
    pub fn opposite(self) -> Self {
        match self {
            Self::Enabled => Self::Disabled,
            Self::Disabled => Self::Enabled,
        }
    }
}

impl InStable for Stable {
    type Transition = Transition;

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

/// [`MediaExchangeState`] in transition to another
/// [`StableMediaExchangeState`].
///
/// [`StableMediaExchangeState`] which is stored in
/// [`TransitionMediaExchangeState`] variants is a state which we already have,
/// but we still waiting for a desired state update. If desired state update
/// won't be received, then the stored [`StableMediaExchangeState`] will be
/// applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transition {
    /// [`MediaStateControllable`] should be enabled, but awaits server
    /// permission.
    ///
    /// [`MediaStateControllable`]: super::MediaStateControllable
    Enabling(Stable),

    /// [`MediaStateControllable`] should be disabled, but awaits server
    /// permission.
    ///
    /// [`MediaStateControllable`]: super::MediaStateControllable
    Disabling(Stable),
}

impl InTransition for Transition {
    type Stable = Stable;

    /// Returns intention which this [`TransitionMediaExchangeState`] indicates.
    #[inline]
    fn intended(self) -> Self::Stable {
        match self {
            Self::Enabling(_) => Stable::Enabled,
            Self::Disabling(_) => Stable::Disabled,
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

    /// Converts [`TransitionMediaExchangeState`] to the opposite
    /// [`TransitionMediaExchangeState`] with a same inner
    /// [`StableMediaExchangeState`].
    #[inline]
    fn opposite(self) -> Self {
        match self {
            Self::Enabling(stable) => Self::Disabling(stable),
            Self::Disabling(stable) => Self::Enabling(stable),
        }
    }
}
