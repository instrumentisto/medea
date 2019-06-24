//! Definition of the [`ThenAll`] tokio-based combinator,
//! executing each future in a list of futures one by one. Ignores errors.

use std::prelude::v1::*;

use std::fmt;
use std::mem;
use tokio::prelude::future::{IntoFuture, Future};
use tokio::prelude::{Async, Poll};

#[derive(Debug)]
enum ElemState<T> where T: Future {
    Pending(T),
    Done(),
}


#[must_use = "futures do nothing unless polled"]
pub struct ThenAll<I>
    where I: IntoIterator,
          I::Item: IntoFuture,
{
    elems: Vec<ElemState<<I::Item as IntoFuture>::Future>>,
    last_elem: usize,
}

impl<I> fmt::Debug for ThenAll<I>
    where I: IntoIterator,
          I::Item: IntoFuture,
          <<I as IntoIterator>::Item as IntoFuture>::Future: fmt::Debug,
          <<I as IntoIterator>::Item as IntoFuture>::Item: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("ThenAll")
            .field("elems", &self.elems)
            .finish()
    }
}

pub fn then_all<I>(i: I) -> ThenAll <I>
    where I: IntoIterator,
          I::Item: IntoFuture,
{
    let elems = i.into_iter().map(|f| {
        ElemState::Pending(f.into_future())
    }).collect();
    ThenAll { elems, last_elem: 0 }
}

impl<I> Future for ThenAll<I>
    where I: IntoIterator,
          I::Item: IntoFuture,
{
    type Item = ();
    type Error = <I::Item as IntoFuture>::Error;


    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut result = Async::NotReady;

        while (result != Async::Ready(())) {
            let last_val = match self.elems[self.last_elem] {
                ElemState::Pending(ref mut t) => {
                    match t.poll() {
                        Ok(Async::Ready(_)) => Ok(()),
                        Ok(Async::NotReady) => {
                            continue;
                        },
                        Err(e) => Err(()),
                    }
                },
                ElemState::Done() => {
                    Ok(())
                },
            };

            self.elems[self.last_elem] = ElemState::Done();
            self.last_elem += 1;

            if self.last_elem == self.elems.len() {
                result = Async::Ready(());
            }
        }

        Ok(result)
    }
}