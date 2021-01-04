//! Implementation of the [`Future`] extension which implements [`Future`] can
//! check resolve condition and restart themselves if needed.

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::Future;

/// Extension for the [`Future`] which can recheck [`Poll::Ready`] condition
/// after resolving and restart if [`Future`] goes to the [`Poll::Pending`]
/// condition accordingly to the [`RecheckableFutureExt::restart`] method.
///
/// This kind of [`Future`]s should be joined by [`medea_reactive::join_all`]
/// function.
///
/// [`medea_reactive::join_all`]: crate::join_all
#[allow(clippy::module_name_repetitions)]
pub trait RecheckableFutureExt: Future + Unpin {
    /// Returns `true` if [`RecheckableFutureExt`] matches resolving condition.
    fn is_done(&self) -> bool;

    /// Restart this [`RecheckableFutureExt`]. After this function call, this
    /// [`Future`] can be safely polled.
    fn restart(&mut self);
}

impl<F: ?Sized + RecheckableFutureExt> RecheckableFutureExt for Box<F> {
    fn is_done(&self) -> bool {
        <F as RecheckableFutureExt>::is_done(&*self)
    }

    fn restart(&mut self) {
        <F as RecheckableFutureExt>::restart(&mut *self)
    }
}

/// [`Future`] which joins [`RecheckableFutureExt`].
///
/// [`JoinRecheckableCounterFuture`] will check that all
/// [`RecheckableFutureExt`] are stay done after all [`RecheckableFutureExt`]
/// was resolved. If some [`RecheckableFutureExt`] is undone then this
/// [`Future`] will wait for resolve.
#[derive(Debug)]
pub struct JoinRecheckableCounterFuture<F> {
    /// List of [`Poll::Pending`] [`RecheckableFutureExt`]s.
    pending: Vec<F>,

    /// List of [`Poll::Ready`] [`RecheckableFutureExt`]s.
    done: Vec<F>,
}

impl<F: Unpin + RecheckableFutureExt> JoinRecheckableCounterFuture<F> {
    /// Returns [`Future`] which will be resolved when all provided
    /// [`RecheckableFutureExt`]s will be resolved and done.
    fn new(pending: Vec<F>) -> Self {
        Self {
            pending,
            done: Vec::new(),
        }
    }

    /// Polls all [`JoinRecheckableCounterFuture::pending`] [`Future`]s. If
    /// [`Future`] returned [`Poll::Ready`] then moves this [`Future`] to the
    /// [`JoinRecheckableCounterFuture::done`].
    fn poll_pending(self: &mut Pin<&mut Self>, cx: &mut Context<'_>) {
        let mut i = 0;
        while i != self.pending.len() {
            match Pin::new(&mut self.pending[0]).as_mut().poll(cx) {
                Poll::Ready(_) => {
                    let done = self.pending.remove(i);
                    self.done.push(done);
                }
                Poll::Pending => {
                    i += 1;
                }
            }
        }
    }

    /// Rechecks all [`JoinRecheckableCounterFuture::done`] [`Future`]s and if
    /// [`Future`] is not done, restarts it, moves it to the
    /// [`JoinRecheckableCounterFuture::pending`].
    ///
    /// If at least one [`Future`] moved from
    /// [`JoinRecheckableCounterFuture::done`] to the
    /// [`JoinRecheckableCounterFuture::pending`] then `false` will be returned.
    fn recheck_done_futures(&mut self) -> bool {
        let mut is_ready = true;
        let mut i = 0;
        while i != self.done.len() {
            if self.done[i].is_done() {
                i += 1;
            } else {
                let mut pending = self.done.remove(i);
                pending.restart();
                self.pending.push(pending);
                is_ready = false;
            }
        }

        is_ready
    }
}

impl<F: RecheckableFutureExt> RecheckableFutureExt
    for JoinRecheckableCounterFuture<F>
{
    fn is_done(&self) -> bool {
        !self.done.iter().any(|f| !f.is_done())
    }

    fn restart(&mut self) {
        let _ = self.recheck_done_futures();
    }
}

impl<F: RecheckableFutureExt> Future for JoinRecheckableCounterFuture<F> {
    type Output = ();

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        self.poll_pending(cx);

        if self.pending.is_empty() && self.recheck_done_futures() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

/// Joins provided [`Vec`] of [`RecheckableFutureExt`].
///
/// Returned [`Future`] will be resolved when all [`Future`]s returned
/// [`Poll::Ready`] and all [`RecheckableFutureExt::is_done`] returns `true`.
#[must_use]
pub fn join_all<F: RecheckableFutureExt>(
    futs: Vec<F>,
) -> JoinRecheckableCounterFuture<F> {
    JoinRecheckableCounterFuture::new(futs)
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc, time::Duration};

    use futures::{executor, poll, task::Poll, FutureExt, StreamExt};
    use tokio::{
        task::{spawn_local, LocalSet},
        time::delay_for,
    };

    use crate::ProgressableCell;

    use super::*;

    macro_rules! impl_future {
        ($name:ty => $output:expr) => {
            impl Future for $name {
                type Output = ();

                fn poll(
                    self: Pin<&mut Self>,
                    _: &mut Context<'_>,
                ) -> Poll<Self::Output> {
                    $output
                }
            }
        };
    }

    #[test]
    fn doesnt_resolves_if_not_done() {
        executor::block_on(async {
            struct Fut;
            impl_future!(Fut => Poll::Ready(()));

            impl RecheckableFutureExt for Fut {
                fn is_done(&self) -> bool {
                    false
                }

                fn restart(&mut self) {}
            }

            assert_eq!(poll!(join_all(vec![Fut])), Poll::Pending);
        })
    }

    #[test]
    fn resolved_if_done() {
        executor::block_on(async {
            struct Fut;
            impl_future!(Fut => Poll::Ready(()));

            impl RecheckableFutureExt for Fut {
                fn is_done(&self) -> bool {
                    true
                }

                fn restart(&mut self) {}
            }

            assert_eq!(poll!(join_all(vec![Fut])), Poll::Ready(()));
        })
    }

    #[test]
    fn doesnt_resolved_if_one_fut_is_not_done() {
        executor::block_on(async {
            struct Fut(bool);
            impl_future!(Fut => Poll::Ready(()));

            impl RecheckableFutureExt for Fut {
                fn is_done(&self) -> bool {
                    self.0
                }

                fn restart(&mut self) {}
            }

            assert_eq!(
                poll!(join_all(vec![Fut(false), Fut(true)])),
                Poll::Pending
            );
        })
    }

    #[test]
    fn resolves_if_all_done() {
        executor::block_on(async {
            struct Fut;
            impl_future!(Fut => Poll::Ready(()));

            impl RecheckableFutureExt for Fut {
                fn is_done(&self) -> bool {
                    true
                }

                fn restart(&mut self) {}
            }

            assert_eq!(poll!(join_all(vec![Fut, Fut, Fut])), Poll::Ready(()));
        })
    }

    #[test]
    fn doesnt_restart_futs_until_all_resolved() {
        executor::block_on(async {
            struct Fut;
            impl_future!(Fut => Poll::Pending);

            impl RecheckableFutureExt for Fut {
                fn is_done(&self) -> bool {
                    unreachable!(
                        "This function shouldn't be called during this test"
                    )
                }

                fn restart(&mut self) {
                    unreachable!(
                        "This function shouldn't be called during this test"
                    )
                }
            }

            assert_eq!(poll!(join_all(vec![Fut])), Poll::Pending)
        })
    }

    #[test]
    fn restart_fut_on_undone() {
        executor::block_on(async {
            struct Fut(Rc<Cell<bool>>);
            impl_future!(Fut => Poll::Ready(()));

            impl RecheckableFutureExt for Fut {
                fn is_done(&self) -> bool {
                    false
                }

                fn restart(&mut self) {
                    self.0.set(true);
                }
            }

            let is_restart_called = Rc::new(Cell::new(false));
            let fut =
                join_all(vec![Fut(Rc::clone(&is_restart_called))]).shared();
            assert_eq!(poll!(fut.clone()), Poll::Pending);
            assert!(is_restart_called.get());
            assert_eq!(poll!(fut), Poll::Pending,);
        })
    }

    /// Checks that two joined with [`join_all`]
    /// [`ProgressableCell::when_all_processed`]'s [`RecheckableFutureExt`]s
    /// will be resolved only if they both processed at the end.
    #[tokio::test]
    async fn join_all_works_correctly_with_when_processed() {
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
                        delay_for(Duration::from_millis(100)).await;
                        assert_eq!(
                            DELAYED_PROCESSED_UPDATE,
                            updatable_cell_rx
                                .select_next_some()
                                .await
                                .into_inner()
                        );
                    }
                });

                join_all(vec![
                    updatable_cell.when_all_processed(),
                    ProgressableCell::new(0).when_all_processed(),
                ])
                .await;
                assert!(
                    matches!(
                        futures::poll!(updatable_cell.when_all_processed()),
                        Poll::Ready(_),
                    ),
                    "ProgressableCell is not processed, but `join_all` resolved."
                );
            })
            .await;
    }
}
