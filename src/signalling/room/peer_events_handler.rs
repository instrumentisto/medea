//! Handlers for [`PeerStarted`] and [`PeerStopped`] messages emitted by
//! [`PeerTrafficWatcher`].
//!
//! [`PeerTrafficWatcher`]: crate::signalling::peers::PeerTrafficWatcher

use std::iter;

use actix::{AsyncContext, Handler, StreamHandler};
use chrono::{DateTime, Utc};

use crate::{
    api::control::callback::MediaType,
    log::prelude::*,
    media::PeerStateMachine,
    signalling::{
        elements::endpoints::Endpoint,
        peers::{PeerStarted, PeerStopped, PeersMetricsEventHandler},
    },
};

use super::Room;
use crate::{
    api::control::callback::MediaDirection,
    signalling::{
        elements::endpoints::webrtc::WebRtcPublishEndpoint,
        peers::PeersMetricsEvent, room::ActFuture,
    },
};
use medea_client_api_proto::PeerId;

impl Handler<PeerStarted> for Room {
    type Result = ();

    /// Updates [`Peer`]s publishing status of the [`WebRtcPublishEndpoint`], if
    /// [`WebRtcPublishEndpoint`] have only one publishing [`Peer`] and
    /// `on_start` callback is set then `on_start` will be sent to the
    /// Control API.
    ///
    /// If [`WebRtcPlayEndpoint`]'s `on_start` callback is set then `on_start`
    /// will be sent to the Control API.
    fn handle(
        &mut self,
        msg: PeerStarted,
        _: &mut Self::Context,
    ) -> Self::Result {
        let peer_id = msg.0;
        for endpoint in self.peers.get_endpoints_by_peer_id(peer_id) {
            match endpoint {
                Endpoint::WebRtcPublishEndpoint(publish) => {
                    publish.set_peer_status(peer_id, true);
                    if publish.publishing_peers_count() == 1 {
                        if let Some((url, req)) =
                            publish.get_on_start(Utc::now(), MediaType::Both)
                        {
                            self.callbacks.send_callback(url, req);
                        }
                    }
                }
                Endpoint::WebRtcPlayEndpoint(play) => {
                    if let Some((url, req)) =
                        play.get_on_start(Utc::now(), MediaType::Both)
                    {
                        self.callbacks.send_callback(url, req);
                    }
                }
            }
        }
    }
}

impl Handler<PeerStopped> for Room {
    type Result = ();

    /// Updates [`Peer`]s publishing state of all endpoints related to stopped
    /// [`Peer`].
    ///
    /// `on_stop` callback will be sent for all endpoints which considered as
    /// stopped and haves `on_stop` callback set.
    fn handle(
        &mut self,
        msg: PeerStopped,
        _: &mut Self::Context,
    ) -> Self::Result {
        let peer_id = msg.peer_id;
        let at = msg.at;
        debug!("Peer [id = {}] was stopped at {}", peer_id, at);

        if let Ok(peer) = self.peers.get_peer_by_id(peer_id) {
            peer.endpoints()
                .into_iter()
                .filter_map(|e| {
                    e.get_traffic_not_flowing_on_stop(peer.id(), at)
                })
                .chain(
                    self.peers
                        .get_peer_by_id(peer.partner_peer_id())
                        .map(PeerStateMachine::endpoints)
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|e| {
                            e.get_traffic_not_flowing_on_stop(
                                peer.partner_peer_id(),
                                at,
                            )
                        }),
                )
                .for_each(|(url, req)| {
                    self.callbacks.send_callback(url, req);
                });
        }
    }
}

impl StreamHandler<PeersMetricsEvent> for Room {
    fn handle(&mut self, event: PeersMetricsEvent, ctx: &mut Self::Context) {
        ctx.spawn(event.dispatch_with(self));
    }
}

impl PeersMetricsEventHandler for Room {
    type Output = ActFuture<()>;

    /// Notifies [`Room`] about [`PeerConnection`]'s partial media traffic
    /// stopping.
    #[allow(clippy::filter_map)]
    fn on_no_traffic_flow(
        &mut self,
        peer_id: PeerId,
        was_flowing_at: DateTime<Utc>,
        media_type: MediaType,
        _: MediaDirection,
    ) -> Self::Output {
        debug!("NoTrafficFlow for Peer [id = {}].", peer_id);
        let peer = self.peers.get_peer_by_id(peer_id).unwrap();

        peer.endpoints()
            .into_iter()
            .filter_map(|e| e.upgrade())
            .filter_map(|e| match e {
                Endpoint::WebRtcPublishEndpoint(publish) => Some(publish),
                _ => None,
            })
            .flat_map(|e: WebRtcPublishEndpoint| {
                iter::once(e.get_on_stop(peer_id, was_flowing_at, media_type))
                    .chain(
                        e.sinks()
                            .into_iter()
                            .map(|e| e.get_on_stop(was_flowing_at, media_type)),
                    )
                    .filter_map(|e| e)
            })
            .for_each(|(url, req)| {
                self.callbacks.send_callback(url, req);
            });

        Box::new(actix::fut::ready(()))
    }

    /// Notifies [`Room`] about [`PeerConnection`]'s partial traffic starting.
    #[allow(clippy::filter_map)]
    fn on_traffic_flows(
        &mut self,
        peer_id: PeerId,
        media_type: MediaType,
        _: MediaDirection,
    ) -> Self::Output {
        debug!("TrafficFlows for Peer [id = {}].", peer_id);
        let peer = self.peers.get_peer_by_id(peer_id).unwrap();

        peer.endpoints()
            .into_iter()
            .filter_map(|e| e.upgrade())
            .filter_map(|e| match e {
                Endpoint::WebRtcPublishEndpoint(publish) => Some(publish),
                _ => None,
            })
            .flat_map(|e: WebRtcPublishEndpoint| {
                iter::once(e.get_on_start(Utc::now(), media_type))
                    .chain(
                        e.sinks()
                            .into_iter()
                            .map(|e| e.get_on_start(Utc::now(), media_type)),
                    )
                    .filter_map(|e| e)
            })
            .for_each(|(url, req)| {
                self.callbacks.send_callback(url, req);
            });

        Box::new(actix::fut::ready(()))
    }
}
