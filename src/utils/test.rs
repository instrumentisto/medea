//! Utils which can be used for the unit testing purposes.
//!
//! This module is available only in the unit tests.

use std::time::Duration;

use futures::{future, future::Either, Future};
use tokio::time::delay_for;

/// Starts provided [`Future`] and waits provided [`Duration`] for the
/// [`Future`] result.
///
/// Returns `Ok` with a [`Result`] of the [`Future`] if it finished within
/// [`Duration`] period.
///
/// Returns `Err` if [`Future`] doesn't finished within [`Duration`] period.
pub async fn future_with_timeout<T>(
    fut: impl Future<Output = T>,
    dur: Duration,
) -> Result<T, ()> {
    let result = future::select(Box::pin(fut), Box::pin(delay_for(dur))).await;
    match result {
        Either::Left((res, _)) => Ok(res),
        Either::Right(_) => Err(()),
    }
}
