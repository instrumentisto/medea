use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use futures::{channel::mpsc, future};
use medea_client_api_proto::{IceServer, PeerId};
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;

use crate::{
    media::MediaManager,
    utils::{delay_for, TaskHandle},
};

use super::{PeerConnection, PeerError, PeerEvent};

/// [`PeerConnection`] factory and repository.
#[cfg_attr(feature = "mockable", mockall::automock)]
pub trait PeerRepository {
    /// Creates new [`PeerConnection`] with provided ID and injecting provided
    /// [`IceServer`]s, [`PeerEvent`] sender and stored [`MediaManager`].
    ///
    /// # Errors
    ///
    /// Errors if creating [`PeerConnection`] fails.
    fn create_peer(
        &self,
        id: PeerId,
        ice_servers: Vec<IceServer>,
        events_sender: mpsc::UnboundedSender<PeerEvent>,
        is_force_relayed: bool,
    ) -> Result<Rc<PeerConnection>, Traced<PeerError>>;

    /// Returns [`PeerConnection`] stored in repository by its ID.
    fn get(&self, id: PeerId) -> Option<Rc<PeerConnection>>;

    /// Removes [`PeerConnection`] stored in repository by its ID.
    fn remove(&self, id: PeerId);

    /// Returns all [`PeerConnection`]s stored in repository.
    fn get_all(&self) -> Vec<Rc<PeerConnection>>;
}

/// [`PeerConnection`] factory and repository.
pub struct Repository {
    /// [`MediaManager`] for injecting into new created [`PeerConnection`]s.
    media_manager: Rc<MediaManager>,

    /// Peer id to [`PeerConnection`],
    peers: Rc<RefCell<HashMap<PeerId, Rc<PeerConnection>>>>,

    /// [`TaskHandle`] for a task which will call
    /// [`PeerConnection::send_peer_stats`] of all [`PeerConnection`]s
    /// every second and send updated [`RtcStat`]s to the server.
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
    /// all [`PeerConnection`]s every second and send updated [`RtcStat`]s
    /// to the server.
    fn schedule_peers_stats_scrape(&mut self) {
        let peers = self.peers.clone();
        let (fut, abort) = future::abortable(async move {
            loop {
                delay_for(Duration::from_secs(1).into()).await;

                let peers =
                    peers.borrow().values().cloned().collect::<Vec<_>>();
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
    /// Creates new [`PeerConnection`] with provided ID and injecting provided
    /// [`IceServer`]s, stored [`PeerEvent`] sender and [`MediaManager`].
    fn create_peer(
        &self,
        id: PeerId,
        ice_servers: Vec<IceServer>,
        peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
        is_force_relayed: bool,
    ) -> Result<Rc<PeerConnection>, Traced<PeerError>> {
        let peer = PeerConnection::new(
            id,
            peer_events_sender,
            ice_servers,
            Rc::clone(&self.media_manager),
            is_force_relayed,
        )
        .map_err(tracerr::map_from_and_wrap!())?;
        self.peers.borrow_mut().insert(id, Rc::clone(&peer));
        Ok(peer)
    }

    /// Returns [`PeerConnection`] stored in repository by its ID.
    #[inline]
    fn get(&self, id: PeerId) -> Option<Rc<PeerConnection>> {
        self.peers.borrow().get(&id).cloned()
    }

    /// Removes [`PeerConnection`] stored in repository by its ID.
    #[inline]
    fn remove(&self, id: PeerId) {
        self.peers.borrow_mut().remove(&id);
    }

    /// Returns all [`PeerConnection`]s stored in a repository.
    #[inline]
    fn get_all(&self) -> Vec<Rc<PeerConnection>> {
        self.peers.borrow().values().cloned().collect()
    }
}
