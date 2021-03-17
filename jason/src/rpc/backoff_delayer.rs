//! Delayer that increases delay time by provided multiplier on each call.

use crate::utils::{delay_for, JsDuration};

/// Delayer that increases delay time by provided multiplier on each call.
///
/// Delay time increasing will be stopped when
/// [`BackoffDelayer::current_interval`] reaches
/// [`BackoffDelayer::max_interval`].
///
/// First delay will be [`BackoffDelayer::current_interval`].
pub struct BackoffDelayer {
    /// Delay of next [`BackoffDelayer::delay`] call.
    ///
    /// Will be increased by [`BackoffDelayer::delay`] call.
    current_interval: JsDuration,

    /// Maximum delay for which this [`BackoffDelayer`] may delay.
    max_interval: JsDuration,

    /// The multiplier by which [`BackoffDelayer::current_interval`] will be
    /// multiplied on [`BackoffDelayer::delay`] call.
    interval_multiplier: f32,
}

impl BackoffDelayer {
    /// Creates and returns new [`BackoffDelayer`].
    #[inline]
    #[must_use]
    pub fn new(
        starting_interval: JsDuration,
        interval_multiplier: f32,
        max_interval: JsDuration,
    ) -> Self {
        Self {
            current_interval: starting_interval,
            max_interval,
            interval_multiplier,
        }
    }

    /// Resolves after [`BackoffDelayer::current_interval`] delay.
    ///
    /// Next call of this function will delay
    /// [`BackoffDelayer::current_interval`] *
    /// [`BackoffDelayer::interval_multiplier`] milliseconds,
    /// until [`BackoffDelayer::max_interval`] is reached.
    #[inline]
    pub async fn delay(&mut self) {
        delay_for(self.get_delay()).await;
    }

    /// Returns current interval and increases it for next call.
    #[must_use]
    fn get_delay(&mut self) -> JsDuration {
        if self.is_max_interval_reached() {
            self.max_interval
        } else {
            let delay = self.current_interval;
            self.current_interval =
                self.current_interval * self.interval_multiplier;
            delay
        }
    }

    /// Returns `true` when max delay ([`BackoffDelayer::max_interval`]) is
    /// reached.
    #[must_use]
    fn is_max_interval_reached(&self) -> bool {
        self.current_interval >= self.max_interval
    }
}
