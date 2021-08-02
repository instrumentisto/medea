//! [`Future`] returned from `when_*_processed` methods of progressable
//! containers.

#![allow(clippy::module_name_repetitions)]

use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{
    future::{self, Future, LocalBoxFuture},
    ready, FutureExt as _,
};

/// Factory producing a [`Future`] in [`when_all_processed()`] function.
pub type Factory<'a, T> = Box<dyn Fn() -> LocalBoxFuture<'a, T> + 'static>;

/// Creates [`AllProcessed`] [`Future`] from the provided [`Iterator`] of
/// [`Factory`]s.
pub fn when_all_processed<I, T>(futures: I) -> AllProcessed<'static>
where
    I: IntoIterator<Item = Factory<'static, T>>,
    T: 'static,
{
    #[allow(clippy::needless_collect)]
    let futures: Vec<_> = futures.into_iter().collect();
    AllProcessed::new(Box::new(move || {
        let futures = futures.iter().map(AsRef::as_ref).map(|f| f());
        Box::pin(future::join_all(futures).map(drop))
    }))
}

/// [`Future`] with inner factory. [`Factory`] can be unwrapped using [`Into`]
/// implementation.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Processed<'a, T = ()> {
    /// Factory creating the underlying [`Future`].
    factory: Factory<'a, T>,

    /// Underlying [`Future`] being polled in a [`Future`] implementation.
    fut: LocalBoxFuture<'a, T>,
}

impl<'a, T> Processed<'a, T> {
    /// Creates new [`Processed`] from the provided [`Factory`].
    #[inline]
    pub fn new(factory: Factory<'a, T>) -> Self {
        Self {
            fut: factory(),
            factory,
        }
    }
}

impl<'a, T> Future for Processed<'a, T> {
    type Output = T;

    #[inline]
    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        self.fut.as_mut().poll(cx)
    }
}

impl<'a, T> From<Processed<'a, T>> for Factory<'a, T> {
    #[inline]
    fn from(p: Processed<'a, T>) -> Self {
        p.factory
    }
}

impl<'a, T> fmt::Debug for Processed<'a, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Processed").finish()
    }
}

/// [`Future`] returned by [`when_all_processed()`] function.
///
/// Restarts the underlying [`Future`] when it is ready to recheck that all
/// conditions are still met.
///
/// Inner [`Factory`] can be unwrapped using [`Into`] implementation.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct AllProcessed<'a, T = ()> {
    /// Factory creating the underlying [`Future`] and recreating it to recheck
    /// the [`Future`] during polling.
    factory: Factory<'a, T>,

    /// Underlying [`Future`] being polled in a [`Future`] implementation.
    fut: LocalBoxFuture<'a, T>,
}

impl<'a, T> From<AllProcessed<'a, T>> for Factory<'a, T> {
    #[inline]
    fn from(p: AllProcessed<'a, T>) -> Self {
        p.factory
    }
}

impl<'a, T> AllProcessed<'a, T> {
    /// Creates new [`AllProcessed`] from provided [`Factory`].
    #[inline]
    fn new(factory: Factory<'a, T>) -> Self {
        Self {
            fut: factory(),
            factory,
        }
    }
}

impl<'a, T> fmt::Debug for AllProcessed<'a, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AllProcessed").finish()
    }
}

impl<'a, T> Future for AllProcessed<'a, T> {
    type Output = T;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        drop(ready!(self.fut.as_mut().poll(cx)));

        let mut retry = (self.factory)();
        match retry.as_mut().poll(cx) {
            Poll::Ready(r) => Poll::Ready(r),
            Poll::Pending => {
                self.fut = retry;
                Poll::Pending
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{rc::Rc, time::Duration};

    use futures::{task::Poll, StreamExt};
    use tokio::{
        task::{spawn_local, LocalSet},
        time,
    };

    use crate::ProgressableCell;

    use super::*;

    /// Checks whether two joined [`ProgressableCell::when_all_processed()`]s
    /// will be resolved only if they both processed at the end.
    #[tokio::test]
    async fn when_all_processed_rechecks() {
        LocalSet::new()
            .run_until(async {
                /// Update which will be processed instantly.
                const INSTANT_PROCESSED_UPDATE: u8 = 1;
                /// Update which will be processed after 100 millis.
                const DELAYED_PROCESSED_UPDATE: u8 = 2;

                let updatable_cell = Rc::new(ProgressableCell::new(0));
                let _ = spawn_local({
                    let updatable_cell = Rc::clone(&updatable_cell);
                    let mut updatable_cell_rx =
                        updatable_cell.subscribe().skip(1).fuse();
                    updatable_cell.set(INSTANT_PROCESSED_UPDATE);
                    async move {
                        assert_eq!(
                            INSTANT_PROCESSED_UPDATE,
                            updatable_cell_rx
                                .select_next_some()
                                .await
                                .into_inner()
                        );

                        updatable_cell.set(DELAYED_PROCESSED_UPDATE);
                        time::sleep(Duration::from_millis(100)).await;
                        assert_eq!(
                            DELAYED_PROCESSED_UPDATE,
                            updatable_cell_rx
                                .select_next_some()
                                .await
                                .into_inner()
                        );
                    }
                });

                when_all_processed(vec![
                    updatable_cell.when_all_processed().into(),
                    ProgressableCell::new(0).when_all_processed().into(),
                ])
                .await;
                assert!(
                    matches!(
                        futures::poll!(updatable_cell.when_all_processed()),
                        Poll::Ready(_),
                    ),
                    "ProgressableCell is not processed, but `join_all` \
                     resolved."
                );
            })
            .await;
    }
}
