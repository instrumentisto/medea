//! State of the media mute state.

use super::{InStable, InTransition};

/// State of the media mute state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stable {
    /// [`MediaStateControllable`] is muted.
    Muted,

    /// [`MediaStateControllable`] is unmuted.
    Unmuted,
}

impl Stable {
    /// Returns opposite to this [`Stable`].
    pub fn opposite(self) -> Self {
        match self {
            Self::Muted => Self::Unmuted,
            Self::Unmuted => Self::Muted,
        }
    }
}

impl From<bool> for Stable {
    #[inline]
    fn from(is_muted: bool) -> Self {
        if is_muted {
            Self::Muted
        } else {
            Self::Unmuted
        }
    }
}

impl InStable for Stable {
    type Transition = Transition;

    #[inline]
    fn start_transition(self) -> Self::Transition {
        match self {
            Self::Unmuted => Transition::Muting(self),
            Self::Muted => Transition::Unmuting(self),
        }
    }
}

/// [`MuteState`] in transition to another
/// [`Stable`].
///
/// [`Stable`] which is stored in
/// [`Transition`] variants is a state which we already have,
/// but we still waiting for a desired state update. If desired state update
/// won't be received, then the stored [`Stable`] will be
/// applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transition {
    /// [`MediaStateControllable`] should be muted, but awaits server
    /// permission.
    ///
    /// [`MediaStateControllable`]: super::MediaStateControllable
    Muting(Stable),

    /// [`MediaStateControllable`] should be unmuted, but awaits server
    /// permission.
    ///
    /// [`MediaStateControllable`]: super::MediaStateControllable
    Unmuting(Stable),
}

impl InTransition for Transition {
    type Stable = Stable;

    #[inline]
    fn intended(self) -> Self::Stable {
        match self {
            Self::Unmuting(_) => Stable::Unmuted,
            Self::Muting(_) => Stable::Muted,
        }
    }

    #[inline]
    fn set_inner(self, inner: Self::Stable) -> Self {
        match self {
            Self::Unmuting(_) => Self::Unmuting(inner),
            Self::Muting(_) => Self::Muting(inner),
        }
    }

    #[inline]
    fn into_inner(self) -> Self::Stable {
        match self {
            Self::Unmuting(s) | Self::Muting(s) => s,
        }
    }

    #[inline]
    fn opposite(self) -> Self {
        match self {
            Self::Unmuting(stable) => Self::Muting(stable),
            Self::Muting(stable) => Self::Unmuting(stable),
        }
    }
}
