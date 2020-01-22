//! Helper utils used in project.

use std::pin::Pin;

use actix::prelude::dev::{
    Actor, ActorFuture, Arbiter, AsyncContext, ContextFutureSpawner as _,
    Message, MessageResponse, ResponseChannel, WrapFuture as _,
};
use futures::Future;

/// Creates new [`HashMap`] from a list of key-value pairs.
///
/// # Example
///
/// ```rust
/// # use medea::hashmap;
/// let map = hashmap! {
///     "a" => 1,
///     "b" => 2,
/// };
/// assert_eq!(map["a"], 1);
/// assert_eq!(map["b"], 2);
/// assert_eq!(map.get("c"), None);
/// ```
///
/// [`HashMap`]: std::collections::HashMap
#[macro_export]
macro_rules! hashmap {
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$(hashmap!(@single $rest)),*]));

    ($($key:expr => $value:expr,)+) => { hashmap!($($key => $value),+) };
    ($($key:expr => $value:expr),*) => {
        {
            let _cap = hashmap!(@count $($key),*);
            let mut _map = ::std::collections::HashMap::with_capacity(_cap);
            $(
                let _ = _map.insert($key, $value);
            )*
            _map
        }
    };
}

// TODO: remove after https://github.com/actix/actix/pull/313
/// A specialized future for asynchronous message handling. Exists because
/// [`actix::ResponseFuture`] implements [`actix::dev::MessageResponse`] only
/// for `Output = Result<_, _>`;
pub struct ResponseAnyFuture<T>(pub Pin<Box<dyn Future<Output = T>>>);

// TODO: remove after https://github.com/actix/actix/pull/310
/// A specialized actor future for asynchronous message handling. Exists
/// because [`actix::ResponseActFuture`] implements
/// [`actix::dev::MessageResponse`] only for `Output = Result<_, _>`;
pub struct ResponseActAnyFuture<A, O>(
    pub Box<dyn ActorFuture<Output = O, Actor = A>>,
);

impl<A, M, T: 'static> MessageResponse<A, M> for ResponseAnyFuture<T>
where
    A: Actor,
    M::Result: Send,
    M: Message<Result = T>,
    A::Context: AsyncContext<A>,
{
    fn handle<R: ResponseChannel<M>>(self, _: &mut A::Context, tx: Option<R>) {
        Arbiter::spawn(async move {
            if let Some(tx) = tx {
                tx.send(self.0.await)
            }
        });
    }
}

impl<A, M, O: 'static> MessageResponse<A, M> for ResponseActAnyFuture<A, O>
where
    A: Actor,
    M: Message<Result = O>,
    A::Context: AsyncContext<A>,
{
    fn handle<R: ResponseChannel<M>>(
        self,
        ctx: &mut A::Context,
        tx: Option<R>,
    ) {
        self.0
            .then(move |res, this, _| {
                if let Some(tx) = tx {
                    tx.send(res);
                }
                async {}.into_actor(this)
            })
            .spawn(ctx);
    }
}
