//! Delayer that increases delay time by provided multiplier on each call.
//!
//! Backed by [`ExponentialBackoff`].

use std::time::Duration;

use backoff::{future::Retry, ExponentialBackoff};
use futures::{channel::oneshot, future::BoxFuture, Future};

use crate::platform;

/// [`ExponentialBackoff`] adapted for async runtime used by Jason.
pub struct BackoffDelayer(ExponentialBackoff);

impl BackoffDelayer {
    /// Creates [`BackoffDelayer`] from provided settings.
    #[must_use]
    pub fn new(
        starting_delay_ms: u32,
        multiplier: f64,
        max_delay_ms: u32,
        max_elapsed_time_ms: Option<u32>,
    ) -> BackoffDelayer {
        // max_delay_ms = max_elapsed if max_delay > max_elapsed
        let max_delay = max_elapsed_time_ms
            .map_or(max_delay_ms, |max_elapsed| max_delay_ms.min(max_elapsed));
        // starting_delay = max_delay_ms if starting_delay > max_delay
        let starting_delay_ms = starting_delay_ms.min(max_delay);
        let initial_interval = Duration::from_millis(starting_delay_ms.into());

        BackoffDelayer(ExponentialBackoff {
            current_interval: initial_interval,
            initial_interval,
            randomization_factor: 0.0,
            multiplier,
            max_interval: Duration::from_millis(max_delay.into()),
            max_elapsed_time: max_elapsed_time_ms
                .map(Into::into)
                .map(Duration::from_millis),
            ..ExponentialBackoff::default()
        })
    }

    /// Retries given `operation` according to the [`BackoffDelayer`] policy.
    ///
    /// # Errors
    ///
    /// With error that is returned by the provided `operation`.
    pub async fn retry<Fn, Fut, I, E>(self, operation: Fn) -> Result<I, E>
    where
        Fn: FnMut() -> Fut,
        Fut: Future<Output = Result<I, backoff::Error<E>>>,
    {
        Retry::new(Sleeper, self.0, |_, _| {}, operation).await
    }
}

/// [`backoff::future::Sleeper`] implementation that uses
/// [`platform::delay_for()`].
struct Sleeper;

impl backoff::future::Sleeper for Sleeper {
    type Sleep = BoxFuture<'static, ()>;

    fn sleep(&self, delay: Duration) -> Self::Sleep {
        let (tx, rx) = oneshot::channel();
        platform::spawn(async move {
            platform::delay_for(delay).await;
            let _ = tx.send(());
        });
        Box::pin(async move {
            let _ = rx.await;
        })
    }
}
