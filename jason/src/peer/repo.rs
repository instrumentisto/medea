use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use futures::{channel::mpsc, future};
use medea_client_api_proto::PeerId;
use medea_macro::{watch, watchers};
use medea_reactive::ObservableHashMap;
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;

use crate::{
    api::{Connections, RoomError},
    media::{LocalTracksConstraints, MediaManager},
    peer,
    utils::{component, delay_for, TaskHandle},
};

use super::{PeerConnection, PeerError, PeerEvent};

/// Component responsible for the new [`PeerComponent`] creating and removing.
pub type Component = component::Component<PeersState, Peers>;

impl Component {
    /// asdasd
    pub fn new(
        repo: peer::repo::Repository,
        peer_event_sender: mpsc::UnboundedSender<PeerEvent>,
        send_constraints: LocalTracksConstraints,
        connections: Rc<Connections>,
    ) -> Self {
        spawn_component!(
            Self,
            Rc::new(PeersState::default()),
            Rc::new(Peers {
                repo,
                peer_event_sender,
                send_constraints,
                connections,
            }),
        )
    }

    /// Returns [`PeerConnection`] stored in repository by its ID.
    #[inline]
    pub fn get(&self, id: PeerId) -> Option<Rc<PeerConnection>> {
        self.repo.get(id)
    }

    /// Returns all [`PeerConnection`]s stored in a repository.
    #[inline]
    pub fn get_all(&self) -> Vec<Rc<PeerConnection>> {
        self.repo.get_all()
    }

    /// Stops all timeouts in the all [`PeerComponent`]s.
    #[inline]
    pub fn stop_timeouts(&self) {
        self.repo.stop_timeouts()
    }

    /// Resumes all timeouts in the all [`PeerComponent`]s.
    #[inline]
    pub fn resume_timeouts(&self) {
        self.repo.resume_timeouts()
    }
}

/// State of the [`PeersComponent`].
#[derive(Default)]
pub struct PeersState(RefCell<ObservableHashMap<PeerId, Rc<peer::State>>>);

/// Context of the [`PeersComponent`].
pub struct Peers {
    /// [`PeerComponent`] repository.
    ///
    /// [`PeerComponent`]: crate::peer::PeerComponent
    repo: peer::repo::Repository,

    /// Channel for send events produced [`PeerConnection`] to [`Room`].
    ///
    /// [`PeerConnection`]: crate::peer::PeerConnection;
    peer_event_sender: mpsc::UnboundedSender<PeerEvent>,

    /// Constraints to local [`local::Track`]s that are being published by
    /// [`PeerConnection`]s in this [`Room`].
    ///
    /// [`PeerConnection`]: crate::peer::PeerConnection;
    send_constraints: LocalTracksConstraints,

    /// Collection of [`Connection`]s with a remote `Member`s.
    ///
    /// [`Connection`]: crate::api::Connection
    connections: Rc<Connections>,
}

impl PeersState {
    pub fn insert(&self, peer_id: PeerId, peer_state: peer::State) {
        self.0.borrow_mut().insert(peer_id, Rc::new(peer_state));
    }

    pub fn get(&self, peer_id: PeerId) -> Option<Rc<peer::State>> {
        self.0.borrow().get(&peer_id).cloned()
    }

    pub fn remove(&self, peer_id: PeerId) {
        self.0.borrow_mut().remove(&peer_id);
    }
}

#[watchers]
impl Component {
    /// Watches for new [`PeerState`] insertions.
    ///
    /// Creates new [`PeerComponent`] based on the inserted [`PeerState`].
    ///
    /// [`PeerState`]: crate::peer::PeerState
    /// [`PeerComponent`]: crate::peer::PeerComponent
    #[watch(self.state().0.borrow().on_insert())]
    #[inline]
    async fn insert_peer_watcher(
        peers: Rc<Peers>,
        _: Rc<PeersState>,
        (peer_id, new_peer): (PeerId, Rc<peer::State>),
    ) -> Result<(), Traced<RoomError>> {
        peers
            .repo
            .create_peer(
                peer_id,
                new_peer,
                peers.peer_event_sender.clone(),
                peers.send_constraints.clone(),
                Rc::clone(&peers.connections),
            )
            .map_err(tracerr::map_from_and_wrap!())?;

        Ok(())
    }

    /// Watches for [`PeerState`] remove.
    ///
    /// Removes [`PeerComponent`] and closes [`Connection`] by
    /// [`Connections::close_connection`] call.
    #[watch(self.state().0.borrow().on_remove())]
    #[inline]
    async fn remove_peer_watcher(
        peers: Rc<Peers>,
        _: Rc<PeersState>,
        (peer_id, _): (PeerId, Rc<peer::State>),
    ) -> Result<(), Traced<RoomError>> {
        peers.repo.remove(peer_id);
        peers.connections.close_connection(peer_id);

        Ok(())
    }
}

/// [`PeerConnection`] factory and repository.
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
    stats_scrape_task: Option<TaskHandle>,
}

impl Repository {
    /// Instantiates new [`Repository`] with a given [`MediaManager`].
    #[inline]
    pub fn new(media_manager: Rc<MediaManager>) -> Self {
        let mut this = Self {
            media_manager,
            peers: Rc::new(RefCell::new(HashMap::new())),
            stats_scrape_task: None,
        };
        this.schedule_peers_stats_scrape();

        this
    }

    /// Creates new [`PeerComponent`] with a provided [`PeerState`].
    ///
    /// # Errors
    ///
    /// Errors if creating [`PeerState`] fails.
    #[inline]
    fn create_peer(
        &self,
        peer_id: PeerId,
        state: Rc<peer::State>,
        peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
        send_constraints: LocalTracksConstraints,
        connections: Rc<Connections>,
    ) -> Result<(), Traced<PeerError>> {
        let peer = PeerConnection::new(
            peer_id,
            peer_events_sender,
            state.ice_servers().clone(),
            Rc::clone(&self.media_manager),
            state.force_relay(),
            send_constraints,
            connections,
        )
        .map_err(tracerr::map_from_and_wrap!())?;

        let component = spawn_component!(peer::Component, state, peer);
        self.peers.borrow_mut().insert(peer_id, component);

        Ok(())
    }

    /// Returns [`PeerConnection`] stored in repository by its ID.
    #[inline]
    fn get(&self, id: PeerId) -> Option<Rc<PeerConnection>> {
        self.peers.borrow().get(&id).map(component::Component::ctx)
    }

    /// Removes [`PeerConnection`] stored in repository by its ID.
    #[inline]
    fn remove(&self, id: PeerId) {
        self.peers.borrow_mut().remove(&id);
    }

    /// Returns all [`PeerConnection`]s stored in a repository.
    #[inline]
    fn get_all(&self) -> Vec<Rc<PeerConnection>> {
        self.peers
            .borrow()
            .values()
            .map(component::Component::ctx)
            .collect()
    }

    /// Stops all timeouts in the all [`PeerComponent`]s.
    #[inline]
    fn stop_timeouts(&self) {
        for peer in self.peers.borrow().values() {
            peer.stop_state_transitions_timers();
            peer.state().stop_timeouts();
        }
    }

    /// Resumes all timeouts in the all [`PeerComponent`]s.
    #[inline]
    fn resume_timeouts(&self) {
        for peer in self.peers.borrow().values() {
            peer.reset_state_transitions_timers();
            peer.state().resume_timeouts();
        }
    }

    /// Schedules task which will call [`PeerConnection::send_peer_stats`] of
    /// all [`PeerConnection`]s every second and send updated [`RtcStats`]
    /// to the server.
    ///
    /// [`RtcStats`]: crate::peer::RtcStats
    fn schedule_peers_stats_scrape(&mut self) {
        let peers = self.peers.clone();
        let (fut, abort) = future::abortable(async move {
            loop {
                delay_for(Duration::from_secs(1).into()).await;

                let peers = peers
                    .borrow()
                    .values()
                    .map(component::Component::ctx)
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
        self.stats_scrape_task = Some(abort.into());
    }
}
