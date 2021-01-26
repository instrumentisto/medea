//! Implementation of the [`IceCandidate`]s store.

use std::{cell::RefCell, collections::HashSet};

use futures::stream::LocalBoxStream;
use medea_client_api_proto::IceCandidate;
use medea_reactive::ObservableHashSet;

use crate::{
    media::LocalTracksConstraints,
    utils::{AsProtoState, SynchronizableState},
};

/// Store of the all [`IceCandidate`]s of the [`PeerComponent`].
#[derive(Debug)]
pub struct IceCandidates(RefCell<ObservableHashSet<IceCandidate>>);

impl IceCandidates {
    pub fn new() -> Self {
        Self(RefCell::new(ObservableHashSet::new()))
    }

    /// Adds new [`IceCandidate`].
    pub fn add(&self, candidate: IceCandidate) {
        self.0.borrow_mut().insert(candidate);
    }

    /// Returns [`LocalBoxStream`] into which all added [`IceCandidate`]s will
    /// be sent.
    pub fn on_add(&self) -> LocalBoxStream<'static, IceCandidate> {
        self.0.borrow().on_insert()
    }
}

impl SynchronizableState for IceCandidates {
    type Input = HashSet<IceCandidate>;

    fn from_proto(input: Self::Input, _: &LocalTracksConstraints) -> Self {
        Self(RefCell::new(input.into()))
    }

    fn apply(&self, input: Self::Input, _: &LocalTracksConstraints) {
        self.0.borrow_mut().update(input);
    }
}

impl AsProtoState for IceCandidates {
    type Output = HashSet<IceCandidate>;

    fn as_proto(&self) -> Self::Output {
        self.0.borrow().iter().cloned().collect()
    }
}
