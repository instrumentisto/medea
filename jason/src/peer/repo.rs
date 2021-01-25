//! Component responsible for the [`peer::Component`] creating and removing.

use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use futures::{
    channel::mpsc, future, future::LocalBoxFuture, FutureExt as _, TryFutureExt,
};
use medea_client_api_proto::{self as proto, MediaSourceKind, PeerId};
use medea_macro::watchers;
use medea_reactive::ObservableHashMap;
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;

use crate::{
    api::{Connections, RoomError},
    media::{
        track::local, LocalTracksConstraints, MediaManager, RecvConstraints,
    },
    peer,
    peer::PeerError,
    utils::{
        component, delay_for, AsProtoState, JasonError, SynchronizableState,
        TaskHandle,
    },
    MediaKind,
};

use super::{PeerConnection, PeerEvent};

/// Component responsible for the [`peer::Component`] creating and removing.
pub type Component = component::Component<State, Repository>;

impl Component {
    /// Returns [`PeerConnection`] stored in the repository by its ID.
    #[inline]
    #[must_use]
    pub fn get(&self, id: PeerId) -> Option<Rc<PeerConnection>> {
        self.peers.borrow().get(&id).map(component::Component::obj)
    }

    /// Returns all [`PeerConnection`]s stored in the repository.
    #[inline]
    #[must_use]
    pub fn get_all(&self) -> Vec<Rc<PeerConnection>> {
        self.peers
            .borrow()
            .values()
            .map(component::Component::obj)
            .collect()
    }

    /// Stops all timeouts in the all [`peer::Component`]s.
    #[inline]
    pub fn stop_timeouts(&self) {
        for peer in self.peers.borrow().values() {
            peer.stop_state_transitions_timers();
            peer.state().stop_timeouts();
        }
    }

    /// Resumes all timeouts in the all [`peer::Component`]s.
    #[inline]
    pub fn resume_timeouts(&self) {
        for peer in self.peers.borrow().values() {
            peer.reset_state_transitions_timers();
            peer.state().resume_timeouts();
        }
    }
}

/// State of the [`Component`].
#[derive(Default)]
pub struct State(RefCell<ObservableHashMap<PeerId, Rc<peer::State>>>);

/// Context of the [`Component`].
pub struct Repository {
    /// [`MediaManager`] for injecting into new created [`PeerConnection`]s.
    media_manager: Rc<MediaManager>,

    /// Peer id to [`PeerConnection`],
    peers: Rc<RefCell<HashMap<PeerId, peer::Component>>>,

    /// [`TaskHandle`] for a task which will call
    /// [`PeerConnection::send_peer_stats`] of all [`PeerConnection`]s
    /// every second and send updated [`PeerMetrics::RtcStats`] to the server.
    ///
    /// [`PeerMetrics::RtcStats`]:
    /// medea_client_api_proto::PeerMetrics::RtcStats
    _stats_scrape_task: TaskHandle,

    /// Channel for sending events produced by [`PeerConnection`] to [`Room`].
    ///
    /// [`PeerConnection`]: crate::peer::PeerConnection
    /// [`Room`]: crate::api::Room
    peer_event_sender: mpsc::UnboundedSender<PeerEvent>,

    /// Constraints to local [`local::Track`]s that are being published by
    /// [`PeerConnection`]s from this [`Repository`].
    ///
    /// [`PeerConnection`]: crate::peer::PeerConnection
    /// [`Room`]: crate::api::Room
    /// [`local::Track`]: crate::media::track::local::Track
    send_constraints: LocalTracksConstraints,

    /// Collection of [`Connection`]s with a remote `Member`s.
    ///
    /// [`Connection`]: crate::api::Connection
    connections: Rc<Connections>,

    /// Constraints to the [`remote::Track`] received by [`PeerConnection`]s
    /// from this [`Repository`].
    ///
    /// Used to disable or enable media receiving.
    ///
    /// [`remote::Track`]: crate::media::track::remote::Track
    recv_constraints: Rc<RecvConstraints>,
}

impl Repository {
    /// Returns new empty [`Repository`].
    ///
    /// Spawns [`RtcStats`] scrape task.
    ///
    /// [`RtcStats`]: crate::peer::RtcStats
    #[must_use]
    pub fn new(
        media_manager: Rc<MediaManager>,
        peer_event_sender: mpsc::UnboundedSender<PeerEvent>,
        send_constraints: LocalTracksConstraints,
        recv_constraints: Rc<RecvConstraints>,
        connections: Rc<Connections>,
    ) -> Self {
        let peers = Rc::default();
        Self {
            media_manager,
            _stats_scrape_task: Self::spawn_peers_stats_scrape_task(Rc::clone(
                &peers,
            )),
            peers,
            peer_event_sender,
            send_constraints,
            recv_constraints,
            connections,
        }
    }

    /// Spawns task which will call [`PeerConnection::send_peer_stats`] of
    /// all [`PeerConnection`]s every second and send updated [`RtcStats`]
    /// to the server.
    ///
    /// Returns [`TaskHandle`] which will stop this task on [`Drop::drop()`].
    ///
    /// [`RtcStats`]: crate::peer::RtcStats
    fn spawn_peers_stats_scrape_task(
        peers: Rc<RefCell<HashMap<PeerId, peer::Component>>>,
    ) -> TaskHandle {
        let (fut, abort) = future::abortable(async move {
            loop {
                delay_for(Duration::from_secs(1).into()).await;

                let peers = peers
                    .borrow()
                    .values()
                    .map(component::Component::obj)
                    .collect::<Vec<_>>();
                future::join_all(
                    peers.iter().map(|p| p.scrape_and_send_peer_stats()),
                )
                .await;
            }
        });

        spawn_local(async move {
            fut.await.ok();
        });

        abort.into()
    }

    /// Returns [`local::TrackHandle`]s for the provided [`MediaKind`] and
    /// [`MediaSourceKind`].
    ///
    /// If [`MediaSourceKind`] is [`None`] then [`local::TrackHandle`]s for all
    /// needed [`MediaSourceKind`]s will be returned.
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::MediaManagerError`] if failed to obtain
    /// [`local::TrackHandle`] from the [`MediaManager`].
    ///
    /// Errors with [`RoomError::PeerConnectionError`] if failed to get
    /// [`MediaStreamSettings`].
    ///
    /// [`MediaStreamSettings`]: crate::MediaStreamSettings
    pub async fn get_local_track_handles(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<Vec<local::TrackHandle>, Traced<RoomError>> {
        let requests: Vec<_> = self
            .peers
            .borrow()
            .values()
            .filter_map(|p| p.get_media_settings(kind, source_kind).transpose())
            .collect::<Result<Vec<_>, _>>()
            .map_err(tracerr::map_from_and_wrap!())?;

        let mut tracks_handles = Vec::new();
        for req in requests {
            let tracks = self
                .media_manager
                .get_tracks(req)
                .await
                .map_err(tracerr::map_from_and_wrap!())
                .map_err(|e| {
                    let _ = self.peer_event_sender.unbounded_send(
                        PeerEvent::FailedLocalMedia {
                            error: JasonError::from(e.clone()),
                        },
                    );

                    e
                })?;
            for (track, is_new) in tracks {
                if is_new {
                    let _ = self.peer_event_sender.unbounded_send(
                        PeerEvent::NewLocalTrack {
                            local_track: Rc::clone(&track),
                        },
                    );
                }
                tracks_handles.push(track.into());
            }
        }

        Ok(tracks_handles)
    }
}

impl State {
    /// Inserts the provided [`peer::State`].
    #[inline]
    pub fn insert(&self, peer_id: PeerId, peer_state: peer::State) {
        self.0.borrow_mut().insert(peer_id, Rc::new(peer_state));
    }

    /// Lookups [`peer::State`] by the provided [`PeerId`].
    #[inline]
    #[must_use]
    pub fn get(&self, peer_id: PeerId) -> Option<Rc<peer::State>> {
        self.0.borrow().get(&peer_id).cloned()
    }

    /// Removes [`peer::State`] with the provided [`PeerId`].
    #[inline]
    pub fn remove(&self, peer_id: PeerId) {
        self.0.borrow_mut().remove(&peer_id);
    }

    /// Updates this [`PeersState`] with a provided
    /// [`proto::state::Room`].
    pub fn apply(
        &self,
        state: proto::state::Room,
        send_cons: &LocalTracksConstraints,
    ) {
        self.0.borrow_mut().remove_not_present(&state.peers);

        for (id, peer_state) in state.peers {
            let peer = self.0.borrow().get(&id).cloned();
            if let Some(peer) = peer {
                peer.apply(peer_state, send_cons);
            } else {
                self.0.borrow_mut().insert(
                    id,
                    Rc::new(peer::State::from_proto(peer_state, send_cons)),
                );
            }
        }
    }

    /// Returns [`Future`] which will be resolved when gUM/gDM request for the
    /// provided [`MediaKind`]/[`MediaSourceKind`] will be resolved.
    ///
    /// [`Result`] returned by this [`Future`] will be the same as result of the
    /// gUM/gDM request.
    ///
    /// Returns last known gUM/gDM request's [`Result`], if currently no gUM/gDM
    /// requests are running for the provided [`MediaKind`]/[`MediaSourceKind`].
    ///
    /// If provided [`None`] [`MediaSourceKind`] then result will be for all
    /// [`MediaSourceKind`]s.
    ///
    /// [`Future`]: std::future::Future
    pub fn local_stream_update_result(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> LocalBoxFuture<'static, Result<(), Traced<PeerError>>> {
        Box::pin(
            future::try_join_all(self.0.borrow().values().map(|p| {
                p.local_stream_update_result(kind, source_kind)
                    .map_err(tracerr::map_from_and_wrap!())
            }))
            .map(|r| r.map(|_| ())),
        )
    }
}

impl AsProtoState for State {
    type Output = proto::state::Room;

    fn as_proto(&self) -> Self::Output {
        Self::Output {
            peers: self
                .0
                .borrow()
                .iter()
                .map(|(id, p)| (*id, p.as_proto()))
                .collect(),
        }
    }
}

#[watchers]
impl Component {
    /// Watches for new [`peer::State`] insertions.
    ///
    /// Creates new [`peer::Component`] based on the inserted [`peer::State`].
    #[watch(self.0.borrow().on_insert())]
    async fn peer_added(
        peers: Rc<Repository>,
        _: Rc<State>,
        (peer_id, new_peer): (PeerId, Rc<peer::State>),
    ) -> Result<(), Traced<RoomError>> {
        let peer = peer::Component::new(
            PeerConnection::new(
                &new_peer,
                peers.peer_event_sender.clone(),
                Rc::clone(&peers.media_manager),
                peers.send_constraints.clone(),
                Rc::clone(&peers.connections),
                Rc::clone(&peers.recv_constraints),
            )
            .map_err(tracerr::map_from_and_wrap!())?,
            new_peer,
        );

        peers.peers.borrow_mut().insert(peer_id, peer);

        Ok(())
    }

    /// Watches for [`peer::State`] removal.
    ///
    /// Removes [`peer::Component`] and closes [`Connection`] by calling
    /// [`Connections::close_connection()`].
    #[inline]
    #[watch(self.0.borrow().on_remove())]
    async fn peer_removed(
        peers: Rc<Repository>,
        _: Rc<State>,
        (peer_id, _): (PeerId, Rc<peer::State>),
    ) -> Result<(), Traced<RoomError>> {
        peers.peers.borrow_mut().remove(&peer_id);
        peers.connections.close_connection(peer_id);
        Ok(())
    }
}
