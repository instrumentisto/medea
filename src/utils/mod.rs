//! Helper utils used in project.

mod actix_try_join_all;

use std::time::Instant;

use chrono::{DateTime, Utc};
use derive_more::From;
use futures::channel::mpsc::{TrySendError, UnboundedSender};
use rand::{distributions::Alphanumeric, Rng};

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
                drop(_map.insert($key, $value));
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

/// `?` analog but for the functions which will return boxed [`ActorFuture`].
///
/// [`ActorFuture`]: actix::ActorFuture
#[macro_export]
macro_rules! actix_try {
    ($e:expr) => {
        match $e {
            Ok(p) => p,
            Err(e) => {
                return Box::pin(actix::fut::err(e.into()));
            }
        };
    };
}

/// Converts provided [`Instant`] into [`chrono::DateTime`].
#[inline]
#[must_use]
pub fn instant_into_utc(instant: Instant) -> DateTime<Utc> {
    chrono::Duration::from_std(instant.elapsed())
        .map_or_else(|_| Utc::now(), |dur| Utc::now() - dur)
}

/// Generates random alphanumeric string of the specified `length`.
#[must_use]
pub fn generate_token(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

/// Cloneable oneshot sender backed by [`UnboundedSender`].
#[derive(Clone, Debug, From)]
pub struct MpscOneshotSender<T>(UnboundedSender<T>);

impl<T> MpscOneshotSender<T> {
    /// Sends the given `message` consuming `self`.
    ///
    /// # Errors
    ///
    /// If receiving side was dropped.
    pub fn send(self, message: T) -> Result<(), TrySendError<T>> {
        self.0.unbounded_send(message)
    }
}
