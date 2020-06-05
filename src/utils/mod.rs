//! Helper utils used in project.

mod actix_try_join_all;

use std::{future::Future, pin::Pin, time::Instant};

use actix::prelude::dev::{
    Actor, ActorFuture, Arbiter, AsyncContext, ContextFutureSpawner as _,
    Message, MessageResponse, ResponseChannel, WrapFuture as _,
};
use chrono::{DateTime, Utc};
use futures::future;

pub use self::actix_try_join_all::actix_try_join_all;

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

/// Creates new [`HashSet`] from a list of values.
///
/// # Example
///
/// ```rust
/// # use medea::hashset;
/// let map = hashset![1, 1, 2];
/// assert!(map.contains(&1));
/// assert!(map.contains(&2));
/// ```
///
/// [`HashSet`]: std::collections::HashSet
#[macro_export]
macro_rules! hashset {
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$(hashset!(@single $rest)),*]));

    ($($value:expr,)+) => { hashset!($($value),+) };
    ($($value:expr),*) => {
        {
            let _cap = hashset!(@count $($value),*);
            let mut _map = ::std::collections::HashSet::with_capacity(_cap);
            $(
                let _ = _map.insert($value);
            )*
            _map
        }
    };
}

/// Generates [`Debug`] implementation for a provided structure with name of
/// this structure.
///
/// In debug print of this structure will be printed just a name of the provided
/// structure.
///
/// # Example
///
/// ```
/// # use medea::impl_debug_by_struct_name;
/// struct Foo;
///
/// impl_debug_by_struct_name!(Foo);
///
/// assert_eq!(format!("{:?}", Foo), "Foo")
/// ```
///
/// [`Debug`]: std::fmt::Debug
#[macro_export]
macro_rules! impl_debug_by_struct_name {
    ($mock:ty) => {
        impl ::std::fmt::Debug for $mock {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> ::std::result::Result<(), ::std::fmt::Error> {
                f.debug_struct(stringify!($mock)).finish()
            }
        }
    };
}

// TODO: remove after https://github.com/actix/actix/pull/313
/// Specialized future for asynchronous message handling. Exists because
/// [`actix::ResponseFuture`] implements [`actix::dev::MessageResponse`] only
/// for `Output = Result<_, _>`.
pub struct ResponseAnyFuture<T>(pub Pin<Box<dyn Future<Output = T>>>);

// TODO: remove after https://github.com/actix/actix/pull/310
/// Specialized actor future for asynchronous message handling. Exists because
/// [`actix::ResponseActFuture`] implements [`actix::dev::MessageResponse`] only
/// for `Output = Result<_, _>`.
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
                future::ready(()).into_actor(this)
            })
            .spawn(ctx);
    }
}

/// Converts provided [`Instant`] into [`chrono::DateTime`].
pub fn instant_into_utc(instant: Instant) -> DateTime<Utc> {
    chrono::Duration::from_std(instant.elapsed())
        .map_or_else(|_| Utc::now(), |dur| Utc::now() - dur)
}
