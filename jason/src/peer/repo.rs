use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use futures::{channel::mpsc, future};
use medea_client_api_proto::PeerId;
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;

use crate::{
    media::{LocalTracksConstraints, MediaManager},
    peer::{component::PeerComponent, PeerState},
    utils::{delay_for, Component, TaskHandle},
};
use crate::api::Connections;

use super::{PeerConnection, PeerError, PeerEvent};

/// [`PeerConnection`] factory and repository.
#[cfg_attr(feature = "mockable", mockall::automock)]
pub trait PeerRepository {
    /// Creates new [`PeerComponent`] with a provided [`PeerState`].
    ///
    /// # Errors
    ///
    /// Errors if creating [`PeerState`] fails.
    fn create_peer(
        &self,
        peer_id: PeerId,
        state: Rc<PeerState>,
        events_sender: mpsc::UnboundedSender<PeerEvent>,
        local_stream_constraints: LocalTracksConstraints,
        connections: Rc<Connections>,
    ) -> Result<(), Traced<PeerError>>;

    /// Returns [`PeerConnection`] stored in repository by its ID.
    fn get(&self, id: PeerId) -> Option<Rc<PeerConnection>>;

    /// Removes [`PeerConnection`] stored in repository by its ID.
    fn remove(&self, id: PeerId);

    /// Returns all [`PeerConnection`]s stored in repository.
    fn get_all(&self) -> Vec<Rc<PeerConnection>>;

    /// Stops all timeouts in the all [`PeerComponent`]s.
    fn stop_timeouts(&self);

    /// Resumes all timeouts in the all [`PeerComponent`]s.
    fn resume_timeouts(&self);
}

/// [`PeerConnection`] factory and repository.
pub struct Repository {
    /// [`MediaManager`] for injecting into new created [`PeerConnection`]s.
    media_manager: Rc<MediaManager>,

    /// Peer id to [`PeerConnection`],
    peers: Rc<RefCell<HashMap<PeerId, PeerComponent>>>,

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
                    .map(Component::ctx)
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

impl PeerRepository for Repository {
    /// Creates new [`PeerComponent`] with a provided [`PeerState`].
    ///
    /// # Errors
    ///
    /// Errors if creating [`PeerState`] fails.
    #[inline]
    fn create_peer(
        &self,
        peer_id: PeerId,
        state: Rc<PeerState>,
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

        let component = spawn_component!(PeerComponent, state, peer);
        self.peers.borrow_mut().insert(peer_id, component);

        Ok(())
    }

    /// Returns [`PeerConnection`] stored in repository by its ID.
    #[inline]
    fn get(&self, id: PeerId) -> Option<Rc<PeerConnection>> {
        self.peers.borrow().get(&id).map(Component::ctx)
    }

    /// Removes [`PeerConnection`] stored in repository by its ID.
    #[inline]
    fn remove(&self, id: PeerId) {
        self.peers.borrow_mut().remove(&id);
    }

    /// Returns all [`PeerConnection`]s stored in a repository.
    #[inline]
    fn get_all(&self) -> Vec<Rc<PeerConnection>> {
        self.peers.borrow().values().map(Component::ctx).collect()
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
}
