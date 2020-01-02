//! Implementation of delayer which increases delay time by provided multiplier
//! on every delay call.

use derive_more::{Display, From};

use crate::utils::{resolve_after, JsCaused, JsDuration, JsError};

/// Delayer which increases delay time by provided multiplier on every delay
/// call
///
/// Delay time increasing will be stopped when [`BackoffDelayer::max_interval`]
/// milliseconds of `current_delay` will be reached. First delay will be
/// [`BackoffDelayer::current_interval`].
pub struct BackoffDelayer {
    /// Delay of next [`BackoffDelayer::delay`] call.
    ///
    /// Will be increased by [`BackoffDelayer::delay`] call.
    current_interval: JsDuration,

    /// Max delay for which this [`BackoffDelayer`] may delay.
    max_interval: JsDuration,

    /// The multiplier by which [`BackoffDelayer::current_interval`] will be
    /// multiplied on [`BackoffDelayer::delay`] call.
    interval_multiplier: f32,
}

impl BackoffDelayer {
    /// Returns new [`BackoffDelayer`].
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
    /// [`BackoffDelayer::interval_multiplier`] milliseconds.
    pub async fn delay(&mut self) {
        resolve_after(self.get_delay()).await;
    }

    /// Returns current interval and increases it for next call.
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
    fn is_max_interval_reached(&self) -> bool {
        self.current_interval >= self.max_interval
    }
}
