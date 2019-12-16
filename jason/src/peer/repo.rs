use std::{collections::HashMap, rc::Rc};

use futures::{channel::mpsc, future::LocalBoxFuture, Future};
use medea_client_api_proto::{IceServer, PeerId};
use std::pin::Pin;
use tracerr::Traced;
use wasm_bindgen::JsValue;

use crate::media::MediaManager;

use super::{PeerConnection, PeerError, PeerEvent};

/// [`PeerConnection`] factory and repository.
#[cfg_attr(feature = "mockable", mockall::automock)]
pub trait PeerRepository {
    /// Creates new [`PeerConnection`] with provided ID and injecting provided
    /// [`IceServer`]s, [`PeerEvent`] sender and stored [`MediaManager`].
    ///
    /// [`PeerConnection`] can be created with muted audio or video [`Track`]s.
    fn create_peer(
        &mut self,
        id: PeerId,
        ice_servers: Vec<IceServer>,
        events_sender: mpsc::UnboundedSender<PeerEvent>,
        enabled_audio: bool,
        enabled_video: bool,
    ) -> Result<Rc<PeerConnection>, Traced<PeerError>>;

    /// Returns [`PeerConnection`] stored in repository by its ID.
    fn get(&self, id: PeerId) -> Option<Rc<PeerConnection>>;

    /// Removes [`PeerConnection`] stored in repository by its ID.
    fn remove(&mut self, id: PeerId);

    /// Returns all [`PeerConnection`]s stored in repository.
    fn get_all(&self) -> Vec<Rc<PeerConnection>>;

    /// Returns future which resolves into [RTCStatsReport][1]
    /// for all [RtcPeerConnection][2]s from this [`PeerRepository`].
    ///
    /// [1]: https://developer.mozilla.org/en-US/docs/Web/API/RTCStatsReport
    /// [2]: https://developer.mozilla.org/en-US/docs/Web/API/RTCPeerConnection
    fn get_stats_for_all_peer_connections(
        &self,
    ) -> LocalBoxFuture<'static, Vec<Result<JsValue, Traced<PeerError>>>>;
}

/// [`PeerConnection`] factory and repository.
pub struct Repository {
    /// [`MediaManager`] for injecting into new created [`PeerConnection`]s.
    media_manager: Rc<MediaManager>,

    /// Peer id to [`PeerConnection`],
    peers: HashMap<PeerId, Rc<PeerConnection>>,
}

impl Repository {
    /// Instantiates new [`Repository`] with a given [`MediaManager`].
    #[inline]
    pub fn new(media_manager: Rc<MediaManager>) -> Self {
        Self {
            media_manager,
            peers: HashMap::new(),
        }
    }
}

impl PeerRepository for Repository {
    /// Creates new [`PeerConnection`] with provided ID and injecting provided
    /// [`IceServer`]s, stored [`PeerEvent`] sender and [`MediaManager`].
    fn create_peer(
        &mut self,
        id: PeerId,
        ice_servers: Vec<IceServer>,
        peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
        enabled_audio: bool,
        enabled_video: bool,
    ) -> Result<Rc<PeerConnection>, Traced<PeerError>> {
        let peer = Rc::new(
            PeerConnection::new(
                id,
                peer_events_sender,
                ice_servers,
                Rc::clone(&self.media_manager),
                enabled_audio,
                enabled_video,
            )
            .map_err(tracerr::map_from_and_wrap!())?,
        );
        self.peers.insert(id, peer);
        Ok(self.peers.get(&id).cloned().unwrap())
    }

    /// Returns future which resolves into [RTCStatsReport][1]
    /// for all [RtcPeerConnection][2]s from this [`PeerRepository`].
    ///
    /// [1]: https://developer.mozilla.org/en-US/docs/Web/API/RTCStatsReport
    /// [2]: https://developer.mozilla.org/en-US/docs/Web/API/RTCPeerConnection
    fn get_stats_for_all_peer_connections(
        &self,
    ) -> LocalBoxFuture<'static, Vec<Result<JsValue, Traced<PeerError>>>> {
        let futs: Vec<_> = self
            .peers
            .values()
            .cloned()
            .map(|p| Box::pin(async move { p.get_stats().await }))
            .collect();

        Box::pin(futures::future::join_all(futs))
    }

    /// Returns [`PeerConnection`] stored in repository by its ID.
    #[inline]
    fn get(&self, id: PeerId) -> Option<Rc<PeerConnection>> {
        self.peers.get(&id).cloned()
    }

    /// Removes [`PeerConnection`] stored in repository by its ID.
    #[inline]
    fn remove(&mut self, id: PeerId) {
        self.peers.remove(&id);
    }

    /// Returns all [`PeerConnection`]s stored in repository.
    #[inline]
    fn get_all(&self) -> Vec<Rc<PeerConnection>> {
        self.peers.values().cloned().collect()
    }
}
