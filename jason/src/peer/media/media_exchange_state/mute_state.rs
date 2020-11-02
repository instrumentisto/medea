use crate::peer::media::{InTransition, InStable};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableMuteState {
    Muted,
    Unmuted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionMuteState {
    Muting(StableMuteState),
    Unmuting(StableMuteState),
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

impl InTransition for TransitionMuteState {
    type Stable = StableMuteState;

    /// Returns intention which this [`MediaExchangeStateTransition`] indicates.
    #[inline]
    fn intended(self) -> Self::Stable {
        match self {
            Self::Unmuting(_) => StableMuteState::Unmuted,
            Self::Muting(_) => StableMuteState::Muted,
        }
    }

    /// Sets inner [`StableMediaExchangeState`].
    #[inline]
    fn set_inner(self, inner: Self::Stable) -> Self {
        match self {
            Self::Unmuting(_) => Self::Unmuting(inner),
            Self::Muting(_) => Self::Muting(inner),
        }
    }

    /// Returns inner [`StableMediaExchangeState`].
    #[inline]
    fn into_inner(self) -> Self::Stable {
        match self {
            Self::Unmuting(s) | Self::Muting(s) => s,
        }
    }

    #[inline]
    fn reverse(self) -> Self {
        match self {
            Self::Unmuting(stable) => Self::Muting(stable),
            Self::Muting(stable) => Self::Unmuting(stable),
        }
    }
}

// impl MediaExchangeStateTransition {
//     /// Returns intention which this [`MediaExchangeStateTransition`] indicates.
//     #[inline]
//     pub fn intended(self) -> StableMediaExchangeState {
//         match self {
//             Self::Enabling(_) => StableMediaExchangeState::Enabled,
//             Self::Disabling(_) => StableMediaExchangeState::Disabled,
//         }
//     }
//
//     /// Sets inner [`StableMediaExchangeState`].
//     #[inline]
//     pub fn set_inner(self, inner: StableMediaExchangeState) -> Self {
//         match self {
//             Self::Enabling(_) => Self::Enabling(inner),
//             Self::Disabling(_) => Self::Disabling(inner),
//         }
//     }
//
//     /// Returns inner [`StableMediaExchangeState`].
//     #[inline]
//     pub fn into_inner(self) -> StableMediaExchangeState {
//         match self {
//             Self::Enabling(s) | Self::Disabling(s) => s,
//         }
//     }
// }

impl InStable for StableMuteState {
    type Transition = TransitionMuteState;

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
            Self::Unmuted => TransitionMuteState::Muting(self),
            Self::Muted => TransitionMuteState::Unmuting(self),
        }
    }
}
