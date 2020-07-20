//! Reactive mutable data containers.

#![deny(
    intra_doc_link_resolution_failure,
    missing_debug_implementations,
    nonstandard_style,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts
)]
#![forbid(unsafe_code)]
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

pub mod collections;
pub mod field;

#[doc(inline)]
pub use crate::{
    collections::{ObservableHashMap, ObservableHashSet, ObservableVec},
    field::{
        cell::ObservableCell, DroppedError, MutObservableFieldGuard,
        Observable, ObservableField, OnObservableFieldModification,
        Subscribable, UniversalSubscriber, Whenable,
    },
};
