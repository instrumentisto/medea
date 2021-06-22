//! Delayer that increases delay time by a provided multiplier on each call,
//! backed by an [`ExponentialBackoff`].

use std::time::Duration;

use backoff::{future::Retry, ExponentialBackoff};
use futures::{channel::oneshot, future::BoxFuture, Future, FutureExt as _};

use crate::platform;

/// [`ExponentialBackoff`] adapted for the used async runtime.
pub struct BackoffDelayer(ExponentialBackoff);

impl BackoffDelayer {
    /// Creates a new [`BackoffDelayer`] out of the provided options.
    #[must_use]
    pub fn new(
        initial_interval: Duration,
        multiplier: f64,
        max_interval: Duration,
        max_elapsed_time: Option<Duration>,
    ) -> BackoffDelayer {
        // max_interval = max_elapsed if max_delay > max_elapsed
        let max_interval = max_elapsed_time
            .map_or(max_interval, |max_elapsed| max_interval.min(max_elapsed));
        // initial_interval = max_interval if initial_interval > max_delay
        let initial_interval = initial_interval.min(max_interval);

        BackoffDelayer(ExponentialBackoff {
            current_interval: initial_interval,
            initial_interval,
            randomization_factor: 0.0,
            multiplier,
            max_interval,
            max_elapsed_time,
            ..ExponentialBackoff::default()
        })
    }

    /// Retries the given `operation` according to this [`BackoffDelayer`]'s
    /// policy.
    ///
    /// # Errors
    ///
    /// Propagates the error returned by the provided `operation`.
    #[inline]
    pub async fn retry<Fn, Fut, I, E>(self, operation: Fn) -> Result<I, E>
    where
        Fn: FnMut() -> Fut,
        Fut: Future<Output = Result<I, backoff::Error<E>>>,
    {
        Retry::new(Sleeper, self.0, |_, _| {}, operation).await
    }
}

/// [`backoff::future::Sleeper`] implementation using [`platform::delay_for()`].
struct Sleeper;

impl backoff::future::Sleeper for Sleeper {
    type Sleep = BoxFuture<'static, ()>;

    fn sleep(&self, delay: Duration) -> Self::Sleep {
        let (tx, rx) = oneshot::channel();
        platform::spawn(async move {
            platform::delay_for(delay).await;
            let _ = tx.send(());
        });
        Box::pin(rx.map(drop))
    }
}
