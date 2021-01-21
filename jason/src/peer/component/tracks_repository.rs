use std::{cell::RefCell, collections::HashMap, rc::Rc};

use derive_more::From;
use futures::{future::LocalBoxFuture, stream::LocalBoxStream};
use medea_client_api_proto::TrackId;
use medea_reactive::{AllProcessed, Guarded, ProgressableHashMap};

use crate::{
    media::LocalTracksConstraints,
    peer::media::sender,
    utils::{AsProtoState, SynchronizableState, Updatable},
};

use super::receiver;

/// Repository for the all [`sender::State`]s/[`receiver::State`]s of the
/// [`PeerComponent`].
#[derive(Debug, From)]
pub struct TracksRepository<S: 'static>(
    RefCell<ProgressableHashMap<TrackId, Rc<S>>>,
);

impl<S> TracksRepository<S> {
    /// Returns new [`TracksRepository`] with a provided tracks.
    pub fn new() -> Self {
        Self(RefCell::new(ProgressableHashMap::new()))
    }

    /// Returns [`Future`] which will be resolved when all inserts/removes will
    /// be processed.
    pub fn when_all_processed(&self) -> AllProcessed<'static> {
        self.0.borrow().when_all_processed()
    }

    /// Inserts provided track.
    pub fn insert(&self, id: TrackId, track: Rc<S>) {
        self.0.borrow_mut().insert(id, track);
    }

    /// Returns track with a provided [`TrackId`].
    pub fn get(&self, id: TrackId) -> Option<Rc<S>> {
        self.0.borrow().get(&id).cloned()
    }

    /// Returns [`Stream`] into which all [`TracksRepository::insert`]ions will
    /// be sent.
    pub fn on_insert(
        &self,
    ) -> LocalBoxStream<'static, Guarded<(TrackId, Rc<S>)>> {
        self.0.borrow().on_insert_with_replay()
    }
}

#[cfg(feature = "mockable")]
impl<S> TracksRepository<S> {
    pub fn when_insert_processed(&self) -> medea_reactive::Processed<'static> {
        self.0.borrow().when_insert_processed()
    }
}

impl TracksRepository<sender::State> {
    /// Returns all [`sender::State`]s which are requires local `MediaStream`
    /// update.
    #[inline]
    pub fn get_outdated(&self) -> Vec<Rc<sender::State>> {
        self.0
            .borrow()
            .values()
            .filter(|s| s.is_local_stream_update_needed())
            .cloned()
            .collect()
    }

    #[inline]
    pub fn connection_lost(&self) {
        self.0.borrow().values().for_each(|s| s.connection_lost());
    }

    #[inline]
    pub fn connection_recovered(&self) {
        self.0
            .borrow()
            .values()
            .for_each(|s| s.connection_recovered());
    }
}

#[cfg(feature = "mockable")]
impl TracksRepository<receiver::State> {
    pub fn stabilize_all(&self) {
        self.0.borrow().values().for_each(|r| r.stabilize());
    }
}

impl TracksRepository<receiver::State> {
    #[inline]
    pub fn connection_lost(&self) {
        self.0.borrow().values().for_each(|s| s.connection_lost());
    }

    #[inline]
    pub fn connection_recovered(&self) {
        self.0
            .borrow()
            .values()
            .for_each(|s| s.connection_recovered());
    }
}

impl<S> SynchronizableState for TracksRepository<S>
where
    S: SynchronizableState,
{
    type Input = HashMap<TrackId, S::Input>;

    fn from_proto(
        input: Self::Input,
        send_cons: &LocalTracksConstraints,
    ) -> Self {
        Self(RefCell::new(
            input
                .into_iter()
                .map(|(id, t)| (id, Rc::new(S::from_proto(t, send_cons))))
                .collect(),
        ))
    }

    fn apply(&self, input: Self::Input, send_cons: &LocalTracksConstraints) {
        self.0.borrow_mut().remove_not_present(&input);

        for (id, track) in input {
            if let Some(sync_track) = self.0.borrow().get(&id) {
                sync_track.apply(track, send_cons);
            } else {
                self.0
                    .borrow_mut()
                    .insert(id, Rc::new(S::from_proto(track, send_cons)));
            }
        }
    }
}

impl<S> Updatable for TracksRepository<S>
where
    S: Updatable,
{
    fn when_stabilized(&self) -> LocalBoxFuture<'static, ()> {
        use futures::FutureExt as _;
        let when = futures::future::join_all(
            self.0.borrow().values().map(|s| s.when_stabilized()),
        );

        Box::pin(when.map(|_| ()))
    }

    fn when_updated(&self) -> AllProcessed<'static> {
        let when_futs: Vec<_> = self
            .0
            .borrow()
            .values()
            .map(|s| s.when_updated().into())
            .collect();

        medea_reactive::when_all_processed(when_futs)
    }
}

impl<S> AsProtoState for TracksRepository<S>
where
    S: AsProtoState,
{
    type Output = HashMap<TrackId, S::Output>;

    fn as_proto(&self) -> Self::Output {
        self.0
            .borrow()
            .iter()
            .map(|(id, s)| (*id, s.as_proto()))
            .collect()
    }
}
