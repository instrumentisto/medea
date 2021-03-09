//! Implementation of a [`IceCandidate`]s store.

use std::{cell::RefCell, collections::HashSet};

use futures::stream::LocalBoxStream;
use medea_client_api_proto::IceCandidate;
use medea_reactive::ObservableHashSet;

use crate::{
    media::LocalTracksConstraints,
    utils::{AsProtoState, SynchronizableState},
};

/// Store of all the [`IceCandidate`]s of a [`peer::Component`].
///
/// [`peer::Component`]: super::Component
#[derive(Debug)]
pub struct IceCandidates(RefCell<ObservableHashSet<IceCandidate>>);

impl IceCandidates {
    /// Returns a new empty [`IceCandidates`] store.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self(RefCell::new(ObservableHashSet::new()))
    }

    /// Adds a new [`IceCandidate`] to this [`IceCandidates`] store.
    #[inline]
    pub fn add(&self, candidate: IceCandidate) {
        self.0.borrow_mut().insert(candidate);
    }

    /// Returns [`LocalBoxStream`] streaming all the added [`IceCandidate`]s.
    #[inline]
    pub fn on_add(&self) -> LocalBoxStream<'static, IceCandidate> {
        self.0.borrow().on_insert()
    }
}

impl SynchronizableState for IceCandidates {
    type Input = HashSet<IceCandidate>;

    #[inline]
    fn from_proto(input: Self::Input, _: &LocalTracksConstraints) -> Self {
        Self(RefCell::new(input.into()))
    }

    #[inline]
    fn apply(&self, input: Self::Input, _: &LocalTracksConstraints) {
        self.0.borrow_mut().update(input);
    }
}

impl AsProtoState for IceCandidates {
    type Output = HashSet<IceCandidate>;

    #[inline]
    fn as_proto(&self) -> Self::Output {
        self.0.borrow().iter().cloned().collect()
    }
}
