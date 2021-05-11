//! [`TryJoinAll`] for [`ActorFuture`].
//!
//! [`ActorFuture`]: actix::ActorFuture
//! [`TryJoinAll`]: futures::future::TryJoinAll

use std::{
    marker::PhantomData,
    mem,
    pin::Pin,
    sync::atomic::AtomicPtr,
    task::{Context, Poll},
};

use actix::{fut::ActorFuture, Actor};

/// Creates a future which represents either a collection of the results of the
/// futures given or an error.
/// The returned future will drive execution for all of its underlying futures,
/// collecting the results into a destination `Vec<T>` in the same order as they
/// were provided.
///
/// If any future returns an error then all other futures will be canceled and
/// an error will be returned immediately. If all futures complete successfully,
/// however, then the returned future will succeed with a [`Vec`] of all the
/// successful results.
///
/// This function is analog for the [`try_join_all`], but for
/// the [`ActorFuture`].
///
/// [`ActorFuture`]: actix::ActorFuture
/// [`TryJoinAll`]: futures::future::TryJoinAll
/// [`try_join_all`]: futures::future::try_join_all
pub fn actix_try_join_all<I, F, T, E, A>(i: I) -> ActixTryJoinAll<F, T, E, A>
where
    I: IntoIterator<Item = F>,
    F: ActorFuture<A, Output = Result<T, E>> + Unpin,
    A: Actor,
{
    let elems: Box<[_]> = i.into_iter().map(ElemState::Pending).collect();
    ActixTryJoinAll {
        elems: elems.into(),
        _actor: PhantomData,
    }
}

#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ActixTryJoinAll<F, T, E, A: Actor>
where
    F: ActorFuture<A, Output = Result<T, E>> + Unpin,
{
    elems: Pin<Box<[ElemState<F, T>]>>,
    _actor: PhantomData<AtomicPtr<A>>,
}

impl<F, T, E, A> ActorFuture<A> for ActixTryJoinAll<F, T, E, A>
where
    F: ActorFuture<A, Output = Result<T, E>> + Unpin,
    A: Actor,
{
    type Output = Result<Vec<T>, E>;

    fn poll(
        mut self: Pin<&mut Self>,
        srv: &mut A,
        ctx: &mut A::Context,
        task: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let mut state = FinalState::AllDone;

        for mut elem in iter_pin_mut(self.elems.as_mut()) {
            if let Some(pending) = elem.as_mut().pending_pin_mut() {
                match pending.poll(srv, ctx, task) {
                    Poll::Pending => state = FinalState::Pending,
                    Poll::Ready(output) => match output {
                        Ok(item) => elem.set(ElemState::Done(Some(item))),
                        Err(e) => {
                            state = FinalState::Error(e);
                            break;
                        }
                    },
                }
            }
        }

        match state {
            FinalState::Pending => Poll::Pending,
            FinalState::AllDone => {
                let mut elems = mem::replace(&mut self.elems, Box::pin([]));
                let results = iter_pin_mut(elems.as_mut())
                    .map(|e| e.take_done().unwrap())
                    .collect();
                Poll::Ready(Ok(results))
            }
            FinalState::Error(e) => {
                drop(mem::replace(&mut self.elems, Box::pin([])));
                Poll::Ready(Err(e))
            }
        }
    }
}

#[derive(Debug)]
enum ElemState<F, T> {
    Pending(F),
    Done(Option<T>),
}

impl<F: Unpin, T> ElemState<F, T> {
    fn pending_pin_mut(self: Pin<&mut Self>) -> Option<Pin<&mut F>> {
        match self.get_mut() {
            ElemState::Pending(f) => Some(Pin::new(f)),
            ElemState::Done(_) => None,
        }
    }

    fn take_done(self: Pin<&mut Self>) -> Option<T> {
        match self.get_mut() {
            ElemState::Pending(_) => None,
            ElemState::Done(output) => output.take(),
        }
    }
}

impl<F: Unpin, T> Unpin for ElemState<F, T> {}

fn iter_pin_mut<T>(slice: Pin<&mut [T]>) -> impl Iterator<Item = Pin<&mut T>>
where
    T: Unpin,
{
    slice.get_mut().iter_mut().map(Pin::new)
}

enum FinalState<E = ()> {
    Pending,
    AllDone,
    Error(E),
}
