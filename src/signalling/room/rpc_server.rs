//! Implementation of the [`RpcServer`] and related [`Command`]s and functions.

use actix::{ActorFuture, Addr, ContextFutureSpawner as _, Handler};
use derive_more::Display;
use failure::Fail;
use futures::future::{FutureExt as _, LocalBoxFuture};
use medea_client_api_proto::{Command, MemberId, PeerId};

use crate::{
    api::{
        client::rpc_connection::{
            AuthorizationError, Authorize, ClosedReason, CommandMessage,
            RpcConnection, RpcConnectionClosed, RpcConnectionEstablished,
            RpcConnectionSettings,
        },
        control::callback::{OnJoinEvent, OnLeaveEvent, OnLeaveReason},
        RpcServer,
    },
    log::prelude::*,
    media::PeerStateMachine,
    signalling::room::RoomError,
    utils::ResponseActAnyFuture,
};

use super::{ActFuture, Room};

/// Error of validating received [`Command`].
#[derive(Debug, Display, Fail, PartialEq)]
pub enum CommandValidationError {
    /// Unable to find expected `Peer`.
    #[display(fmt = "Couldn't find Peer with [id = {}]", _0)]
    PeerNotFound(PeerId),

    /// Specified `Peer` doesn't belong to the `Member` which sends
    /// [`Command`].
    #[display(
        fmt = "Peer [id = {}] that doesn't belong to Member [id = {}]",
        _0,
        _1
    )]
    PeerBelongsToAnotherMember(PeerId, MemberId),
}

impl Room {
    /// Validates given [`CommandMessage`].
    ///
    /// Two assertions are made:
    /// 1. Specified [`PeerId`] must be known to [`Room`].
    /// 2. Found `Peer` must belong to specified `Member`
    fn validate_command(
        &self,
        command: &CommandMessage,
    ) -> Result<(), CommandValidationError> {
        use Command as C;
        use CommandValidationError::{
            PeerBelongsToAnotherMember, PeerNotFound,
        };

        let peer_id = match command.command {
            C::MakeSdpOffer { peer_id, .. }
            | C::MakeSdpAnswer { peer_id, .. }
            | C::SetIceCandidate { peer_id, .. }
            | C::AddPeerConnectionMetrics { peer_id, .. }
            | C::UpdateTracks { peer_id, .. } => peer_id,
        };

        let peer_member_id = self
            .peers
            .map_peer_by_id(peer_id, PeerStateMachine::member_id)
            .map_err(|_| PeerNotFound(peer_id))?;

        if peer_member_id != command.member_id {
            return Err(PeerBelongsToAnotherMember(peer_id, peer_member_id));
        }

        Ok(())
    }
}

impl RpcServer for Addr<Room> {
    /// Sends [`RpcConnectionEstablished`] message to [`Room`] actor propagating
    /// errors.
    fn connection_established(
        &self,
        member_id: MemberId,
        connection: Box<dyn RpcConnection>,
    ) -> LocalBoxFuture<'static, Result<(), ()>> {
        self.send(RpcConnectionEstablished {
            member_id,
            connection,
        })
        .map(|r| {
            r.map_err(|e| {
                error!("Failed to send RpcConnectionEstablished cause {:?}", e)
            })
            .and_then(|r| {
                r.map_err(|e| {
                    error!("RpcConnectionEstablished failed cause: {:?}", e)
                })
            })
        })
        .boxed_local()
    }

    /// Sends [`RpcConnectionClosed`] message to [`Room`] actor ignoring any
    /// errors.
    fn connection_closed(
        &self,
        member_id: MemberId,
        reason: ClosedReason,
    ) -> LocalBoxFuture<'static, ()> {
        self.send(RpcConnectionClosed { member_id, reason })
            .map(|res| {
                if let Err(e) = res {
                    error!("Failed to send RpcConnectionClosed cause {:?}", e,);
                };
            })
            .boxed_local()
    }

    /// Sends [`CommandMessage`] message to [`Room`] actor ignoring any errors.
    fn send_command(
        &self,
        member_id: MemberId,
        msg: Command,
    ) -> LocalBoxFuture<'static, ()> {
        self.send(CommandMessage::new(member_id, msg))
            .map(|res| {
                if let Err(e) = res {
                    error!("Failed to send CommandMessage cause {:?}", e);
                }
            })
            .boxed_local()
    }
}

impl Handler<Authorize> for Room {
    type Result = Result<RpcConnectionSettings, AuthorizationError>;

    /// Responses with `Ok` if `RpcConnection` is authorized, otherwise `Err`s.
    fn handle(
        &mut self,
        msg: Authorize,
        _: &mut Self::Context,
    ) -> Self::Result {
        self.members
            .get_member_by_id_and_credentials(&msg.member_id, &msg.credentials)
            .map(move |member| RpcConnectionSettings {
                idle_timeout: member.get_idle_timeout(),
                ping_interval: member.get_ping_interval(),
            })
    }
}

impl Handler<CommandMessage> for Room {
    type Result = ResponseActAnyFuture<Self, ()>;

    /// Receives [`Command`] from Web client and passes it to corresponding
    /// handlers. Will emit `CloseRoom` on any error.
    fn handle(
        &mut self,
        msg: CommandMessage,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let fut = match self.validate_command(&msg) {
            Ok(_) => match msg.command.dispatch_with(self) {
                Ok(res) => {
                    Box::new(res.then(|res, this, ctx| -> ActFuture<()> {
                        if let Err(e) = res {
                            error!(
                                "Failed handle command, because {}. Room [id \
                                 = {}] will be stopped.",
                                e, this.id,
                            );
                            this.close_gracefully(ctx)
                        } else {
                            Box::new(actix::fut::ready(()))
                        }
                    }))
                }
                Err(err) => {
                    error!(
                        "Failed handle command, because {}. Room [id = {}] \
                         will be stopped.",
                        err, self.id,
                    );
                    self.close_gracefully(ctx)
                }
            },
            Err(err) => {
                warn!(
                    "Ignoring Command from Member [{}] that failed validation \
                     cause: {}",
                    msg.member_id, err
                );
                Box::new(actix::fut::ready(()))
            }
        };
        ResponseActAnyFuture(fut)
    }
}

impl Handler<RpcConnectionEstablished> for Room {
    type Result = ActFuture<Result<(), RoomError>>;

    /// Saves new [`RpcConnection`] in [`ParticipantService`][1], initiates
    /// media establishment between members.
    /// Creates and interconnects all available `Member`'s `Peer`s.
    ///
    /// [`RpcConnection`]: crate::api::client::rpc_connection::RpcConnection
    /// [1]: crate::signalling::participants::ParticipantService
    fn handle(
        &mut self,
        msg: RpcConnectionEstablished,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        info!(
            "RpcConnectionEstablished for Member [id = {}].",
            msg.member_id
        );

        let fut = self
            .members
            .connection_established(ctx, msg.member_id, msg.connection)
            .then(|res, room, _| {
                let member = actix_try!(res);
                Box::new(
                    room.init_member_connections(&member)
                        .map(|res, _, _| res.map(|_| member)),
                )
            })
            .map(|result, room, _| {
                let member = result?;
                if let Some(callback_url) = member.get_on_join() {
                    room.callbacks.send_callback(
                        callback_url,
                        member.get_fid().into(),
                        OnJoinEvent,
                    );
                };
                Ok(())
            });
        Box::new(fut)
    }
}

impl Handler<RpcConnectionClosed> for Room {
    type Result = ();

    /// Passes message to [`ParticipantService`][1] to cleanup stored
    /// connections.
    ///
    /// Removes all related for disconnected `Member` `Peer`s.
    ///
    /// Sends [`PeersRemoved`] message to `Member`.
    ///
    /// Deletes all removed [`PeerId`]s from all `Member`'s endpoints.
    ///
    /// [`PeersRemoved`]: medea-client-api-proto::Event::PeersRemoved
    /// [1]: crate::signalling::participants::ParticipantService
    fn handle(&mut self, msg: RpcConnectionClosed, ctx: &mut Self::Context) {
        info!(
            "RpcConnectionClosed for member {}, reason {:?}",
            msg.member_id, msg.reason
        );

        self.members
            .connection_closed(msg.member_id.clone(), &msg.reason, ctx);

        if let ClosedReason::Closed { normal } = msg.reason {
            if let Some(member) = self.members.get_member_by_id(&msg.member_id)
            {
                if let Some(on_leave_url) = member.get_on_leave() {
                    let reason = if normal {
                        OnLeaveReason::Disconnected
                    } else {
                        OnLeaveReason::LostConnection
                    };
                    self.callbacks.send_callback(
                        on_leave_url,
                        member.get_fid().into(),
                        OnLeaveEvent::new(reason),
                    );
                }
            } else {
                error!(
                    "Member [id = {}] with ID from RpcConnectionClosed not \
                     found.",
                    msg.member_id,
                );
                self.close_gracefully(ctx).spawn(ctx);
            }

            let removed_peers =
                self.peers.remove_peers_related_to_member(&msg.member_id);

            for (peer_member_id, peers_ids) in removed_peers {
                // Here we may have some problems. If two participants
                // disconnect at one moment then sending event
                // to another participant fail,
                // because connection already closed but we don't know about it
                // because message in event loop.
                self.member_peers_removed(peers_ids, peer_member_id, ctx)
                    .map(|_, _, _| ())
                    .spawn(ctx);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::*;

    use crate::{
        api::control::{pipeline::Pipeline, MemberSpec, RoomId, RoomSpec},
        conf::{self, Conf},
        media::peer::tests::dummy_negotiation_sub_mock,
        signalling::{
            participants::ParticipantService,
            peers::{build_peers_traffic_watcher, PeersService},
            room::State,
        },
        AppContext,
    };

    use medea_client_api_proto::IceCandidate;

    fn empty_room() -> Room {
        let room_spec = RoomSpec {
            id: RoomId::from("test"),
            pipeline: Pipeline::new(HashMap::new()),
        };
        let context = AppContext::new(
            Conf::default(),
            crate::turn::new_turn_auth_service_mock(),
        );

        Room {
            id: room_spec.id().clone(),
            peers: PeersService::new(
                room_spec.id().clone(),
                context.turn_service.clone(),
                build_peers_traffic_watcher(&conf::Media::default()),
                &context.config.media,
                dummy_negotiation_sub_mock(),
            ),
            members: ParticipantService::new(&room_spec, &context).unwrap(),
            state: State::Started,
            callbacks: context.callbacks.clone(),
        }
    }

    #[actix_rt::test]
    async fn command_validation_peer_not_found() {
        let mut room = empty_room();

        let member1 = MemberSpec::new(
            Pipeline::new(HashMap::new()),
            String::from("w/e"),
            None,
            None,
            None,
            None,
            None,
        );

        room.members
            .create_member(MemberId(String::from("member1")), &member1)
            .unwrap();

        let no_such_peer = CommandMessage::new(
            MemberId(String::from("member1")),
            Command::SetIceCandidate {
                peer_id: PeerId(1),
                candidate: IceCandidate {
                    candidate: "".to_string(),
                    sdp_m_line_index: None,
                    sdp_mid: None,
                },
            },
        );

        let validation = room.validate_command(&no_such_peer);

        assert_eq!(
            validation,
            Err(CommandValidationError::PeerNotFound(PeerId(1)))
        );
    }

    #[actix_rt::test]
    async fn command_validation_peer_does_not_belong_to_member() {
        let mut room = empty_room();

        let member1 = MemberSpec::new(
            Pipeline::new(HashMap::new()),
            String::from("w/e"),
            None,
            None,
            None,
            None,
            None,
        );

        room.members
            .create_member(MemberId(String::from("member1")), &member1)
            .unwrap();

        let no_such_peer = CommandMessage::new(
            MemberId(String::from("member1")),
            Command::SetIceCandidate {
                peer_id: PeerId(1),
                candidate: IceCandidate {
                    candidate: "".to_string(),
                    sdp_m_line_index: None,
                    sdp_mid: None,
                },
            },
        );

        let validation = room.validate_command(&no_such_peer);

        assert_eq!(
            validation,
            Err(CommandValidationError::PeerNotFound(PeerId(1)))
        );
    }
}
