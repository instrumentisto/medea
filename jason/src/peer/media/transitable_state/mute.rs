//! State of the media mute state.

use super::{InStable, InTransition};

/// State of the media mute state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableMuteState {
    /// [`MediaStateControllable`] is muted.
    Muted,

    /// [`MediaStateControllable`] is unmuted.
    Unmuted,
}

impl StableMuteState {
    /// Returns opposite to this [`StableMuteState`].
    pub fn opposite(self) -> Self {
        match self {
            Self::Muted => Self::Unmuted,
            Self::Unmuted => Self::Muted,
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

impl InStable for StableMuteState {
    type Transition = TransitionMuteState;

    #[inline]
    fn start_transition(self) -> Self::Transition {
        match self {
            Self::Unmuted => TransitionMuteState::Muting(self),
            Self::Muted => TransitionMuteState::Unmuting(self),
        }
    }
}

/// [`MuteState`] in transition to another
/// [`StableMuteState`].
///
/// [`StableMuteState`] which is stored in
/// [`TransitionMuteState`] variants is a state which we already have,
/// but we still waiting for a desired state update. If desired state update
/// won't be received, then the stored [`StableMuteState`] will be
/// applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionMuteState {
    /// [`MediaStateControllable`] should be muted, but awaits server
    /// permission.
    ///
    /// [`MediaStateControllable`]: super::MediaStateControllable
    Muting(StableMuteState),

    /// [`MediaStateControllable`] should be unmuted, but awaits server
    /// permission.
    ///
    /// [`MediaStateControllable`]: super::MediaStateControllable
    Unmuting(StableMuteState),
}

impl InTransition for TransitionMuteState {
    type Stable = StableMuteState;

    #[inline]
    fn intended(self) -> Self::Stable {
        match self {
            Self::Unmuting(_) => StableMuteState::Unmuted,
            Self::Muting(_) => StableMuteState::Muted,
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
