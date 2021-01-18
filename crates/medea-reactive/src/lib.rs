//! Reactive mutable data containers.

#![deny(
    broken_intra_doc_links,
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
pub mod subscribers_store;

#[doc(inline)]
pub use crate::{
    collections::{
        ObservableHashMap, ObservableHashSet, ObservableVec,
        ProgressableHashMap, ProgressableHashSet, ProgressableVec,
    },
    field::{
        cell::ObservableCell, DroppedError, MutObservableFieldGuard,
        Observable, ObservableField, OnObservableFieldModification,
        Progressable, ProgressableCell, UniversalSubscriber, Whenable,
    },
    subscribers_store::progressable::processed::{
        when_all_processed, AllProcessed, Processed,
    },
    subscribers_store::progressable::{Guard, Guarded},
};
