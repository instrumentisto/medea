//! Implementation of delayer which increases delay time by provided multiplier
//! on every delay call.

use derive_more::{Display, From};
use tracerr::Traced;

use crate::utils::{resolve_after, JsCaused, JsDuration, JsError};

/// Errors which can occur in [`BackoffDelayer`].
#[derive(Debug, From, Display, JsCaused)]
pub enum BackoffDelayerError {
    /// Error which can happen while setting JS timer.
    #[display(fmt = "{}", _0)]
    Js(JsError),
}
/// Delayer which increases delay time by provided multiplier on every delay
/// call
///
/// Delay time increasing will be stopped when [`BackoffDelayer::max_delay`]
/// milliseconds of `current_delay` will be reached. First delay will be
/// [`BackoffDelayer::current_delay`].
pub struct BackoffDelayer {
    /// Delay of next [`BackoffDelayer::delay`] call.
    ///
    /// Will be increased by [`BackoffDelayer::delay`] call.
    current_delay: JsDuration,

    /// Max delay for which this [`BackoffDelayer`] may delay.
    max_delay: JsDuration,

    /// The multiplier by which [`BackoffDelayer::current_delay`] will be
    /// multiplied on [`BackoffDelayer::delay`] call.
    multiplier: f32,
}

impl BackoffDelayer {
    /// Returns new [`BackoffDelayer`].
    pub fn new(
        starting_delay_ms: JsDuration,
        multiplier: f32,
        max_delay_ms: JsDuration,
    ) -> Self {
        Self {
            current_delay: starting_delay_ms,
            max_delay: max_delay_ms,
            multiplier,
        }
    }

    /// Returns [`JsDuration`] for a next delay.
    fn get_delay(&mut self) -> JsDuration {
        if self.is_max_delay_reached() {
            self.max_delay
        } else {
            let delay = self.current_delay;
            self.current_delay = self.current_delay * self.multiplier;
            delay
        }
    }

    /// Returns `true` when max delay ([`BackoffDelayer::max_delay`]) is
    /// reached.
    fn is_max_delay_reached(&self) -> bool {
        self.current_delay >= self.max_delay
    }

    /// Resolves after [`BackoffDelayer::current_delay`] delay.
    ///
    /// Next call of this function will delay
    /// [`BackoffDelayer::current_delay`] *
    /// [`BackoffDelayer::multiplier`] milliseconds.
    pub async fn delay(&mut self) -> Result<(), Traced<BackoffDelayerError>> {
        let delay = self.get_delay();
        resolve_after(delay)
            .await
            .map_err(JsError::from)
            .map_err(tracerr::from_and_wrap!())
    }
}
