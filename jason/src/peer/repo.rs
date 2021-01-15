//! Implementation of the component responsible for the [`peer::Component`]
//! creating and removing.

use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use futures::{channel::mpsc, future};
use medea_client_api_proto::PeerId;
use medea_macro::watchers;
use medea_reactive::ObservableHashMap;
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;

use crate::{
    api::{Connections, RoomError},
    media::{LocalTracksConstraints, MediaManager},
    peer,
    utils::{component, delay_for, TaskHandle},
};

use super::{PeerConnection, PeerEvent};
use crate::media::RecvConstraints;

/// Component responsible for the [`peer::Component`] creating and removing.
pub type Component = component::Component<State, Repository>;

impl Component {
    /// Returns [`PeerConnection`] stored in repository by its ID.
    #[inline]
    pub fn get(&self, id: PeerId) -> Option<Rc<PeerConnection>> {
        self.peers.borrow().get(&id).map(component::Component::obj)
    }

    /// Returns all [`PeerConnection`]s stored in a repository.
    #[inline]
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

    /// Channel for send events produced [`PeerConnection`] to [`Room`].
    ///
    /// [`PeerConnection`]: crate::peer::PeerConnection
    /// [`Room`]: crate::api::Room
    peer_event_sender: mpsc::UnboundedSender<PeerEvent>,

    /// Constraints to local [`local::Track`]s that are being published by
    /// [`PeerConnection`]s in this [`Room`].
    ///
    /// [`PeerConnection`]: crate::peer::PeerConnection
    /// [`Room`]: crate::api::Room
    /// [`local::Track`]: crate::media::track::local::Track
    send_constraints: LocalTracksConstraints,

    /// Collection of [`Connection`]s with a remote `Member`s.
    ///
    /// [`Connection`]: crate::api::Connection
    connections: Rc<Connections>,

    recv_constraints: Rc<RecvConstraints>,
}

impl Repository {
    /// Returns new empty [`Repository`].
    ///
    /// Spawns [`RtcStats`] scrape task.
    ///
    /// [`RtcStats`]: crate::peer::RtcStats
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
    /// Returns [`TaskHandle`] which will stop this task on [`Drop::drop`].
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
}

impl State {
    /// Inserts provided [`peer::State`].
    pub fn insert(&self, peer_id: PeerId, peer_state: peer::State) {
        self.0.borrow_mut().insert(peer_id, Rc::new(peer_state));
    }

    /// Lookups [`peer::State`] by provided [`PeerId`].
    pub fn get(&self, peer_id: PeerId) -> Option<Rc<peer::State>> {
        self.0.borrow().get(&peer_id).cloned()
    }

    /// Removes [`peer::State`] with a provided [`PeerId`].
    pub fn remove(&self, peer_id: PeerId) {
        self.0.borrow_mut().remove(&peer_id);
    }
}

#[watchers]
impl Component {
    /// Watches for new [`peer::State`] insertions.
    ///
    /// Creates new [`peer::Component`] based on the inserted [`peer::State`].
    #[watch(self.0.borrow().on_insert())]
    #[inline]
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

    /// Watches for [`peer::State`] remove.
    ///
    /// Removes [`peer::Component`] and closes [`Connection`] by
    /// [`Connections::close_connection`] call.
    #[watch(self.0.borrow().on_remove())]
    #[inline]
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
