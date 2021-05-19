//! Component responsible for the [`peer::Component`] creating and removing.

use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use futures::{channel::mpsc, future};
use medea_client_api_proto::{self as proto, PeerId};
use medea_macro::watchers;
use medea_reactive::ObservableHashMap;
use tracerr::Traced;

use crate::{
    connection::Connections,
    media::{LocalTracksConstraints, MediaManager, RecvConstraints},
    peer, platform,
    room::RoomError,
    utils::{
        component, AsProtoState, SynchronizableState, TaskHandle,
        Updatable as _,
    },
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

    /// Notifies all [`peer::Component`]s about a RPC connection loss.
    #[inline]
    pub fn connection_lost(&self) {
        for peer in self.peers.borrow().values() {
            peer.state().connection_lost();
        }
    }

    /// Notifies all [`peer::Component`]s about a RPC connection restore.
    #[inline]
    pub fn connection_recovered(&self) {
        for peer in self.peers.borrow().values() {
            peer.state().connection_recovered();
        }
    }

    /// Updates this [`State`] with the provided [`proto::state::Room`].
    pub fn apply(&self, new_state: proto::state::Room) {
        let state = self.state();
        let send_cons = &self.obj().send_constraints;

        state.0.borrow_mut().remove_not_present(&new_state.peers);

        for (id, peer_state) in new_state.peers {
            let peer = state.0.borrow().get(&id).cloned();
            if let Some(peer) = peer {
                peer.apply(peer_state, send_cons);
            } else {
                state.0.borrow_mut().insert(
                    id,
                    Rc::new(peer::State::from_proto(peer_state, send_cons)),
                );
            }
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
    /// [`Room`]: crate::room::Room
    peer_event_sender: mpsc::UnboundedSender<PeerEvent>,

    /// Constraints to local [`local::Track`]s that are being published by
    /// [`PeerConnection`]s from this [`Repository`].
    ///
    /// [`Room`]: crate::room::Room
    /// [`local::Track`]: crate::media::track::local::Track
    send_constraints: LocalTracksConstraints,

    /// Collection of [`Connection`]s with a remote `Member`s.
    ///
    /// [`Connection`]: crate::connection::Connection
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
    /// Returns new empty [`platform::RtcStats`].
    ///
    /// Spawns a task for scraping [`platform::RtcStats`].
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

    /// Spawns a task which will call [`PeerConnection::send_peer_stats()`] of
    /// all [`PeerConnection`]s every second and send updated
    /// [`platform::RtcStats`] to a server.
    ///
    /// Returns [`TaskHandle`] which will stop this task on its [`Drop`].
    fn spawn_peers_stats_scrape_task(
        peers: Rc<RefCell<HashMap<PeerId, peer::Component>>>,
    ) -> TaskHandle {
        let (fut, abort) = future::abortable(async move {
            loop {
                platform::delay_for(Duration::from_secs(1)).await;

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

        platform::spawn(async move {
            fut.await.ok();
        });

        abort.into()
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
}

impl AsProtoState for State {
    type Output = proto::state::Room;

    #[inline]
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
            .await
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
    ///
    /// [`Connection`]: crate::connection::Connection
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
