//! Implementation of a store of [`sender::State`]s and [`receiver::State`]s.
//!
//! [`receiver::State`]: super::receiver::State

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use derive_more::From;
use futures::{
    future, future::LocalBoxFuture, stream::LocalBoxStream, FutureExt as _,
    TryFutureExt,
};
use medea_client_api_proto::TrackId;
use medea_reactive::{AllProcessed, Guarded, ProgressableHashMap};
use tracerr::Traced;

use crate::{
    media::LocalTracksConstraints,
    peer::PeerError,
    utils::{AsProtoState, SynchronizableState, Updatable},
};

use super::sender;

/// Repository of all the [`sender::State`]s/[`receiver::State`]s of a
/// [`Component`].
///
/// [`Component`]: super::Component
#[derive(Debug, From)]
pub struct TracksRepository<S: 'static>(
    RefCell<ProgressableHashMap<TrackId, Rc<S>>>,
);

impl<S> TracksRepository<S> {
    /// Creates a new [`TracksRepository`].
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self(RefCell::new(ProgressableHashMap::new()))
    }

    /// Returns [`Future`] resolving once all inserts/removes are processed.
    #[inline]
    pub fn when_all_processed(&self) -> AllProcessed<'static> {
        self.0.borrow().when_all_processed()
    }

    /// Inserts the provided track identified by the given `id`.
    #[inline]
    pub fn insert(&self, id: TrackId, track: Rc<S>) {
        self.0.borrow_mut().insert(id, track);
    }

    /// Returns a track with the provided `id`.
    #[inline]
    #[must_use]
    pub fn get(&self, id: TrackId) -> Option<Rc<S>> {
        self.0.borrow().get(&id).cloned()
    }

    /// Returns a [`Stream`] streaming the all [`TracksRepository::insert`]ions.
    #[inline]
    pub fn on_insert(
        &self,
    ) -> LocalBoxStream<'static, Guarded<(TrackId, Rc<S>)>> {
        self.0.borrow().on_insert_with_replay()
    }
}

impl TracksRepository<sender::State> {
    /// Returns all the [`sender::State`]s which require a local `MediaStream`
    /// update.
    #[inline]
    #[must_use]
    pub fn get_outdated(&self) -> Vec<Rc<sender::State>> {
        self.0
            .borrow()
            .values()
            .filter(|s| s.is_local_stream_update_needed())
            .cloned()
            .collect()
    }

    /// Returns [`Future`] resolving once
    /// [getUserMedia()][1]/[getDisplayMedia()][2] request for the provided
    /// [`TrackId`]s is resolved.
    ///
    /// [`Result`] returned by this [`Future`] will be the same as the result of
    /// the [getUserMedia()][1]/[getDisplayMedia()][2] request.
    ///
    /// Returns last known [getUserMedia()][1]/[getDisplayMedia()][2] request's
    /// [`Result`], if currently no such requests are running for the provided
    /// [`TrackId`]s.
    ///
    /// [`Future`]: std::future::Future
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    pub fn local_stream_update_result(
        &self,
        tracks_ids: HashSet<TrackId>,
    ) -> LocalBoxFuture<'static, Result<(), Traced<PeerError>>> {
        let senders = self.0.borrow();
        Box::pin(
            future::try_join_all(tracks_ids.into_iter().filter_map(|id| {
                Some(
                    senders
                        .get(&id)?
                        .local_stream_update_result()
                        .map_err(tracerr::map_from_and_wrap!()),
                )
            }))
            .map(|r| r.map(|_| ())),
        )
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
    /// Returns [`Future`] resolving once all tracks from this
    /// [`TracksRepository`] will be stabilized meaning that all track's
    /// components won't contain any pending state change transitions.
    fn when_stabilized(&self) -> AllProcessed<'static> {
        let when_futs: Vec<_> = self
            .0
            .borrow()
            .values()
            .map(|s| s.when_stabilized().into())
            .collect();
        medea_reactive::when_all_processed(when_futs)
    }

    /// Returns [`Future`] resolving once all tracks updates are applied.
    ///
    /// [`Future`]: std::future::Future
    fn when_updated(&self) -> AllProcessed<'static> {
        let when_futs: Vec<_> = self
            .0
            .borrow()
            .values()
            .map(|s| s.when_updated().into())
            .collect();
        medea_reactive::when_all_processed(when_futs)
    }

    /// Notifies all the tracks about RPC connection loss.
    #[inline]
    fn connection_lost(&self) {
        self.0.borrow().values().for_each(|s| s.connection_lost());
    }

    /// Notifies all the tracks about RPC connection recovering.
    #[inline]
    fn connection_recovered(&self) {
        self.0
            .borrow()
            .values()
            .for_each(|s| s.connection_recovered());
    }
}

impl<S> AsProtoState for TracksRepository<S>
where
    S: AsProtoState,
{
    type Output = HashMap<TrackId, S::Output>;

    #[inline]
    fn as_proto(&self) -> Self::Output {
        self.0
            .borrow()
            .iter()
            .map(|(id, s)| (*id, s.as_proto()))
            .collect()
    }
}

#[cfg(feature = "mockable")]
impl<S> TracksRepository<S> {
    /// Waits until all the track inserts will be processed.
    #[inline]
    pub fn when_insert_processed(&self) -> medea_reactive::Processed<'static> {
        self.0.borrow().when_insert_processed()
    }
}

#[cfg(feature = "mockable")]
impl TracksRepository<sender::State> {
    /// Sets [`SyncState`] of all [`sender::State`]s to the
    /// [`SyncState::Synced`].
    #[inline]
    pub fn synced(&self) {
        self.0.borrow().values().for_each(|s| s.synced());
    }
}

#[cfg(feature = "mockable")]
impl TracksRepository<super::receiver::State> {
    /// Stabilize all [`receiver::State`] from this [`State`].
    #[inline]
    pub fn stabilize_all(&self) {
        self.0.borrow().values().for_each(|r| r.stabilize());
    }

    /// Sets [`SyncState`] of all [`receiver::State`]s to the
    /// [`SyncState::Synced`].
    #[inline]
    pub fn synced(&self) {
        self.0.borrow().values().for_each(|r| r.synced());
    }
}
