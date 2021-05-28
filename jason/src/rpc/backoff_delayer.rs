//! Delayer that increases delay time by provided multiplier on each call.

use std::time::Duration;

use crate::platform;

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
    current_interval: Duration,

    /// Maximum delay for which this [`BackoffDelayer`] may delay.
    max_interval: Duration,

    /// The multiplier by which [`BackoffDelayer::current_interval`] will be
    /// multiplied on [`BackoffDelayer::delay`] call.
    interval_multiplier: f64,
}

impl BackoffDelayer {
    /// Creates and returns new [`BackoffDelayer`].
    #[inline]
    #[must_use]
    pub fn new(
        starting_interval: Duration,
        interval_multiplier: f64,
        max_interval: Duration,
    ) -> Self {
        Self {
            current_interval: starting_interval,
            max_interval,
            interval_multiplier: interval_multiplier.max(0.0),
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
        platform::delay_for(self.get_delay()).await;
    }

    /// Returns current interval and increases it for next call.
    #[must_use]
    fn get_delay(&mut self) -> Duration {
        if self.is_max_interval_reached() {
            self.max_interval
        } else {
            let delay = self.current_interval;
            self.current_interval = Duration::from_secs_f64(
                self.current_interval.as_secs_f64() * self.interval_multiplier,
            );
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
