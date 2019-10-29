//! Custom error tracing.
//!
//! Provides tools for making error output more informative. It adds ability
//! to capture custom error trace frames and to display errors with final
//! captured trace.
//!
//! # Usage
//!
//! The common rule:
//! - Use macro to capture trace frame in the invocation place.
//!
//! ```
//! use tracerr::Traced;
//!
//! let err = tracerr::new!("my error"); // captures frame
//!
//! let res: Result<(), _> = Err(err)
//!     .map_err(tracerr::wrap!()); // captures frame
//!
//! let err: Traced<&'static str> = res.unwrap_err();
//! assert_eq!(
//!     format!("{}\n{}", err, err.trace()),
//!     r"my error
//! error trace:
//! rust_out
//!   at src/lib.rs:6
//! rust_out
//!   at src/lib.rs:9",
//! );
//!
//! let (val, trace) = err.unwrap();
//! assert_eq!(
//!     format!("{}\n{}", val, trace),
//!     r"my error
//! error trace:
//! rust_out
//!   at src/lib.rs:6
//! rust_out
//!   at src/lib.rs:9",
//! );
//! ```

#![deny(
    missing_debug_implementations,
    nonstandard_style,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code
)]
#![warn(
    deprecated_in_future,
    missing_copy_implementations,
    missing_docs,
    unreachable_pub,
    unused_import_braces,
    unused_labels,
    unused_lifetimes,
    unused_qualifications,
    unused_results
)]

mod trace;

#[cfg(not(feature = "failure"))]
use std::error::Error;
use std::{
    convert::{AsMut, AsRef},
    sync::atomic::{AtomicUsize, Ordering},
};

use derive_more::Display;
#[cfg(feature = "failure")]
use failure::Fail;

pub use self::trace::*;

/// Default capacity for [`Trace`] buffer initialization.
///
/// May be changed if your application requires larger size
/// for better performance and re-allocation avoidance.
pub static DEFAULT_FRAMES_CAPACITY: AtomicUsize = AtomicUsize::new(10);

/// Transparent wrapper for an error which holds captured error trace.
#[derive(Debug, Display)]
#[display(fmt = "{}", err)]
pub struct Traced<E> {
    /// Original error.
    err: E,
    /// Captured error trace.
    trace: Trace,
}

impl<E> Traced<E> {
    /// Unwraps [`Traced`] wrapper by returning contained original error
    /// and captured [`Trace`] separately.
    #[inline]
    pub fn unwrap(self) -> (E, Trace) {
        (self.err, self.trace)
    }

    /// References to the captured [`Trace`].
    ///
    /// This is a raw equivalent of `AsRef<Trace>` (which cannot be implemented
    /// at the moment due to the lack of specialization in Rust).
    // TODO: replace to AsRef<Trace> when Rust will allow specialization
    #[inline]
    pub fn trace(&self) -> &Trace {
        &self.trace
    }
}

impl<E> From<(E, Frame)> for Traced<E> {
    #[inline]
    fn from((err, f): (E, Frame)) -> Self {
        err.wrap_traced(f)
    }
}

impl<E> AsRef<E> for Traced<E> {
    #[inline]
    fn as_ref(&self) -> &E {
        &self.err
    }
}

impl<E> AsMut<E> for Traced<E> {
    #[inline]
    fn as_mut(&mut self) -> &mut E {
        &mut self.err
    }
}

// TODO: use when Rust will allow specialization
// impl<E> AsRef<Trace> for Traced<E> {
// #[inline]
// fn as_ref(&self) -> &Trace {
// &self.trace
// }
// }

#[cfg(not(feature = "failure"))]
impl<E: Error> Error for Traced<E> {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.err.source()
    }
}

#[cfg(feature = "failure")]
impl<E: Fail> Fail for Traced<E> {
    #[inline]
    fn name(&self) -> Option<&str> {
        self.err.name()
    }

    #[inline]
    fn cause(&self) -> Option<&dyn Fail> {
        self.err.cause()
    }

    #[inline]
    fn backtrace(&self) -> Option<&failure::Backtrace> {
        self.err.backtrace()
    }
}

#[allow(clippy::use_self)]
#[cfg(feature = "failure")]
impl<E: Fail> From<Traced<E>> for Box<dyn Fail> {
    #[inline]
    fn from(err: Traced<E>) -> Self {
        Box::new(err)
    }
}

// TODO: use when Rust will allow specialization
// #[cfg(feature = "failure")]
// impl<E: Fail> From<Traced<E>> for Traced<Box<dyn Fail>> {
// #[inline]
// fn from(err: Traced<E>) -> Self {
// unimplemented!()
// }
// }

/// Trait for wrapping errors into [`Traced`] wrapper
/// and growing [`Trace`] inside.
///
/// # Sealed
///
/// This trait is exposed only for being available inside macro invocations,
/// so, outside this library in any code the following MUST BE met:
/// - NEITHER this trait is implemented directly;
/// - NOR its methods are invoked directly.
pub trait WrapTraced<E> {
    /// Wraps given error into `Traced` wrapper with storing given [`Frame`]
    /// of [`Trace`] inside.
    fn wrap_traced(self, f: Frame) -> Traced<E>;
}

impl<E> WrapTraced<E> for E {
    fn wrap_traced(self, f: Frame) -> Traced<Self> {
        let mut trace = Trace::new(Vec::with_capacity(
            DEFAULT_FRAMES_CAPACITY.load(Ordering::Relaxed),
        ));
        trace.push(f);
        Traced { err: self, trace }
    }
}

impl<E> WrapTraced<E> for Traced<E> {
    /// Pushes given [`Frame`] into already captured [`Trace`]
    /// of [`Traced`] wrapper.
    fn wrap_traced(mut self, f: Frame) -> Self {
        self.trace.push(f);
        self
    }
}

/// Maps value of error wrapped in [`Traced`] with its [`From`] implementation.
///
/// This is an equivalent of
/// `impl<E1, E2: From<E1>> From<Traced<E1>> for Traced<E2>`
/// (which cannot be implemented at the moment due to the lack of specialization
/// in Rust).
#[inline]
// TODO: deprecate when Rust will allow specialization
pub fn map_from<F, T: From<F>>(e: Traced<F>) -> Traced<T> {
    Traced {
        err: T::from(e.err),
        trace: e.trace,
    }
}

// TODO: use when Rust will allow specialization
// impl<E1, E2> From<Traced<E1>> for Traced<E2>
// where
// E2: From<E1>,
// {
// fn from(e: Traced<E1>) -> Self {
// unimplemented!()
// }
// }

/// Captures new [`Frame`] in the invocation place and wraps given error
/// into [`Traced`] wrapper containing this [`Frame`]. If the error is
/// a [`Traced`] already then just growth its [`Trace`] with the captured
/// [`Frame`].
///
/// # Examples
///
/// ```
/// use tracerr::Traced;
///
/// let err = 89;
/// let err: Traced<u32> = tracerr::new!(err);
/// let err: Traced<u32> = tracerr::new!(err);
/// ```
#[macro_export]
macro_rules! new {
    ($e:expr) => {
        $crate::WrapTraced::wrap_traced($e, $crate::new_frame!())
    };
}

/// Captures new [`Frame`] in the invocation place and wraps given error
/// into [`Traced`] wrapper containing this [`Frame`] with applying required
/// [`From`] implementation for the wrapped error. If the error is a [`Traced`]
/// already then just applies [`From`] implementation and growth its [`Trace`]
/// with the captured [`Frame`].
///
/// # Examples
///
/// ```
/// use tracerr::Traced;
///
/// let err: Traced<u8> = tracerr::new!(8);
/// let err: Traced<u64> = tracerr::map_from_and_new!(err);
/// ```
#[macro_export]
macro_rules! map_from_and_new {
    ($e:expr) => {
        $crate::new!($crate::map_from($e))
    };
}

/// Provides a closure, which captures new [`Frame`] in the invocation place
/// and wraps given error into [`Traced`] wrapper containing this [`Frame`].
/// If the error is a [`Traced`] already then just growth its [`Trace`]
/// with the captured [`Frame`].
///
/// # Examples
///
/// ```
/// use tracerr::Traced;
///
/// let res: Result<(), u32> = Err(89);
/// let err: Traced<u32> = res
///     .map_err(tracerr::wrap!())
///     .map_err(tracerr::wrap!())
///     .unwrap_err();
/// ```
#[macro_export]
macro_rules! wrap {
    () => ($crate::wrap!(_ => _));
    ($from:ty) => ($crate::wrap!($from => _));
    (=> $to:ty) => ($crate::wrap!(_ => $to));
    ($from:ty => $to:ty) => {
        |err: $from| -> $crate::Traced<$to> {
            $crate::new!(err)
        }
    };
}

/// Provides a closure, which captures new [`Frame`] in the invocation place
/// for the given [`Traced`] wrapper and applies required [`From`]
/// implementation for the wrapped error.
///
/// # Examples
///
/// ```
/// use tracerr::Traced;
///
/// let res: Result<(), Traced<u8>> = Err(tracerr::new!(7));
/// let err: Traced<u64> =
///     res.map_err(tracerr::map_from_and_wrap!()).unwrap_err();
/// ```
#[macro_export]
macro_rules! map_from_and_wrap {
    () => ($crate::map_from_and_wrap!(_ => _));
    ($from:ty) => ($crate::map_from_and_wrap!($from => _));
    (=> $to:ty) => ($crate::map_from_and_wrap!(_ => $to));
    ($from:ty => $to:ty) => {
        |err: $crate::Traced<$from>| -> $crate::Traced<$to> {
            $crate::new!($crate::map_from::<$from, $to>(err))
        }
    };
}

/// Provides a closure, which captures new [`Frame`] in the invocation place,
/// applies required [`From`] implementation for the given error and wraps it
/// into [`Traced`] wrapper containing this [`Frame`].
/// If the error is a [`Traced`] already then just growth its [`Trace`]
/// with the captured [`Frame`].
///
/// # Examples
///
/// ```
/// use tracerr::Traced;
///
/// let res: Result<(), u8> = Err(7);
/// let err: Traced<u64> = res.map_err(tracerr::from_and_wrap!()).unwrap_err();
/// ```
#[macro_export]
macro_rules! from_and_wrap {
    () => ($crate::from_and_wrap!(_ => _));
    ($from:ty) => ($crate::from_and_wrap!($from => _));
    (=> $to:ty) => ($crate::from_and_wrap!(_ => $to));
    ($from:ty => $to:ty) => {
        |err: $from| -> $crate::Traced<$to> {
            $crate::map_from::<$from, $to>($crate::new!(err))
        }
    };
}

#[cfg(test)]
mod new_macro_spec {
    use super::Traced;

    #[test]
    fn creates_new_trace_frame_on_initialization() {
        let err = super::new!(());
        assert_eq!(err.trace.len(), 1, "creates initial frame");
    }

    #[test]
    fn fills_trace_on_subsequent_calls() {
        let err = super::new!(());
        let err = super::new!(err);
        let err = super::new!(err);
        let err: Traced<()> = super::new!(err);
        assert_eq!(err.trace.len(), 4, "trace growths with each call");
    }
}

#[cfg(test)]
mod map_from_and_new_macro_spec {
    use super::Traced;

    #[test]
    fn fills_trace_on_subsequent_calls() {
        let err = super::new!(());
        let err = super::map_from_and_new!(err);
        let err = super::map_from_and_new!(err);
        let err: Traced<()> = super::map_from_and_new!(err);
        assert_eq!(err.trace.len(), 4, "trace growths with each call");
    }

    #[test]
    fn applies_required_from_implementation() {
        let err = super::new!(4u8);
        let err: Traced<u32> = super::map_from_and_new!(err);
        assert!(!err.trace.is_empty(), "captures frames");
    }
}

#[cfg(test)]
mod wrap_macro_spec {
    use super::Traced;

    #[test]
    fn creates_new_trace_frame_on_initialization() {
        let res: Result<(), ()> = Err(());
        let err = res.map_err(super::wrap!()).unwrap_err();
        assert_eq!(err.trace.len(), 1, "creates initial frame");
    }

    #[test]
    fn fills_trace_on_subsequent_calls() {
        let res: Result<(), ()> = Err(());
        let res = res.map_err(super::wrap!());
        let res = res.map_err(super::wrap!());
        let res = res.map_err(super::wrap!(Traced<_>));
        let err = res.map_err(super::wrap!(=> ())).unwrap_err();
        assert_eq!(err.trace.len(), 4, "trace growths with each call");
    }
}

#[cfg(test)]
mod map_from_and_wrap_macro_spec {
    use super::Traced;

    #[test]
    fn fills_trace_on_subsequent_calls() {
        let res: Result<(), ()> = Err(());
        let res = res.map_err(super::wrap!());
        let res = res.map_err(super::map_from_and_wrap!());
        let res = res.map_err(super::map_from_and_wrap!());
        let err = res.map_err(super::map_from_and_wrap!(=> ())).unwrap_err();
        assert_eq!(err.trace.len(), 4, "trace growths with each call");
    }

    #[test]
    fn applies_required_from_implementation() {
        let res: Result<(), u8> = Err(54);
        let res = res.map_err(super::wrap!());
        let err: Traced<u64> =
            res.map_err(super::map_from_and_wrap!()).unwrap_err();
        assert!(!err.trace.is_empty(), "captures frames");
    }
}

#[cfg(test)]
mod from_and_wrap_macro_spec {
    use super::Traced;

    #[test]
    fn fills_trace_on_subsequent_calls() {
        let res: Result<(), ()> = Err(());
        let res = res.map_err(super::wrap!());
        let res = res.map_err(super::from_and_wrap!());
        let res = res.map_err(super::from_and_wrap!());
        let err = res.map_err(super::from_and_wrap!(=> ())).unwrap_err();
        assert_eq!(err.trace.len(), 4, "trace growths with each call");
    }

    #[test]
    fn applies_required_from_implementation() {
        let res: Result<(), u8> = Err(54);
        let err: Traced<u64> =
            res.map_err(super::from_and_wrap!()).unwrap_err();
        assert!(!err.trace.is_empty(), "captures frames");
    }
}
