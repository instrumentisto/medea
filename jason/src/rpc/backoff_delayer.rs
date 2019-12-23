use derive_more::{Display, From};
use js_sys::Promise;
use tracerr::Traced;
use wasm_bindgen_futures::JsFuture;

use crate::utils::{window, JsCaused, JsDuration, JsError};

/// Errors which can occur in [`ProgressiveDelayer`].
#[derive(Debug, From, Display, JsCaused)]
pub enum BackoffDelayerError {
    /// Error which can happen while setting JS timer.
    #[display(fmt = "{}", _0)]
    Js(JsError),
}

/// Delayer which will increase delay time in geometry progression after any
/// `delay` calls.
///
/// Delay time increasing will be stopped when [`ProgressiveDelayer::max_delay`]
/// milliseconds of `current_delay` will be reached. First delay will be
/// [`ProgressiveDelayer::current_delay_ms`].
pub struct BackoffDelayer {
    /// Milliseconds of [`ProgressiveDelayer::delay`] call.
    ///
    /// Will be increased by [`ProgressiveDelayer::delay`] call.
    current_delay: JsDuration,

    /// Max delay for which this [`ProgressiveDelayer`] may delay.
    max_delay: JsDuration,

    /// The multiplier by which [`ProgressiveDelayer::current_delay`] will be
    /// multiplied on [`ProgressiveDelayer::delay`].
    multiplier: f32,
}

impl BackoffDelayer {
    /// Returns new [`ProgressiveDelayer`].
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

    /// Returns next step of delay.
    fn get_delay(&mut self) -> JsDuration {
        if self.is_max_delay_reached() {
            self.max_delay
        } else {
            let delay = self.current_delay;
            self.current_delay = self.current_delay * self.multiplier;
            delay
        }
    }

    /// Returns `true` when max delay ([`ProgressiveDelayer::max_delay_ms`]) is
    /// reached.
    fn is_max_delay_reached(&self) -> bool {
        self.current_delay >= self.max_delay
    }

    /// Resolves after [`ProgressiveDelayer::current_delay`] milliseconds.
    ///
    /// Next call of this function will delay
    /// [`ProgressiveDelayer::current_delay_ms`] *
    /// [`ProgressiveDelayer::multiplier`] milliseconds.
    pub async fn delay(&mut self) -> Result<(), Traced<BackoffDelayerError>> {
        let delay_ms = self.get_delay();
        JsFuture::from(Promise::new(&mut |yes, _| {
            window()
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    &yes,
                    delay_ms.into_js_duration(),
                )
                .unwrap();
        }))
        .await
        .map(|_| ())
        .map_err(JsError::from)
        .map_err(tracerr::from_and_wrap!())
    }
}
