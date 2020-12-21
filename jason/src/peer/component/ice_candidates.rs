use std::{cell::RefCell, collections::HashSet};

use futures::stream::LocalBoxStream;
use medea_client_api_proto::IceCandidate;
use medea_reactive::ObservableHashSet;

use crate::utils::{AsProtoState, SynchronizableState};

#[derive(Debug)]
pub struct IceCandidates(RefCell<ObservableHashSet<IceCandidate>>);

impl IceCandidates {
    pub fn add(&self, candidate: IceCandidate) {
        self.0.borrow_mut().insert(candidate);
    }

    pub fn on_add(&self) -> LocalBoxStream<'static, IceCandidate> {
        self.0.borrow().on_insert()
    }
}

impl SynchronizableState for IceCandidates {
    type Input = HashSet<IceCandidate>;

    fn from_proto(input: Self::Input) -> Self {
        Self(RefCell::new(input.into()))
    }

    fn apply(&self, input: Self::Input) {
        self.0.borrow_mut().update(input);
    }
}

impl AsProtoState for IceCandidates {
    type Output = HashSet<IceCandidate>;

    fn as_proto(&self) -> Self::Output {
        self.0.borrow().iter().cloned().collect()
    }
}
