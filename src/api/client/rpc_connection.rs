//! [`RpcConnection`] with related messages.
use actix::Message;
use futures::Future;

use crate::api::{control::MemberId, protocol::Event};

use std::fmt;

/// Abstraction over RPC connection with some remote [`Member`].
pub trait RpcConnection: fmt::Debug + Send {
    /// Closes [`RpcConnection`].
    /// No [`RpcConnectionClosed`] signals should be emitted.
    /// Always returns success.
    fn close(&mut self) -> Box<dyn Future<Item = (), Error = ()>>;

    /// Sends [`Event`] to remote [`Member`].
    fn send_event(
        &self,
        event: Event,
    ) -> Box<dyn Future<Item = (), Error = ()>>;
}

/// Signal for authorizing new [`RpcConnection`] before establishing.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), AuthorizationError>")]
pub struct Authorize {
    /// ID of [`Member`] to authorize [`RpcConnection`] for.
    pub member_id: MemberId,
    /// Credentials to authorize [`RpcConnection`] with.
    pub credentials: String, // TODO: &str when futures will allow references
}

/// Error of authorization [`RpcConnection`] in [`Room`].
#[derive(Debug)]
pub enum AuthorizationError {
    /// Authorizing [`Member`] does not exists in the [`Room`].
    MemberNotExists,
    /// Provided credentials are invalid.
    InvalidCredentials,
}

/// Signal of new [`RpcConnection`] being established with specified [`Member`].
/// Transport should consider dropping connection if message result is err.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), ()>")]
#[allow(clippy::module_name_repetitions)]
pub struct RpcConnectionEstablished {
    /// ID of [`Member`] that establishes [`RpcConnection`].
    pub member_id: MemberId,
    /// Established [`RpcConnection`].
    pub connection: Box<dyn RpcConnection>,
}
/// Signal of existing [`RpcConnection`] of specified [`Member`] being closed.
#[derive(Debug, Message)]
#[allow(clippy::module_name_repetitions)]
pub struct RpcConnectionClosed {
    /// ID of [`Member`] which [`RpcConnection`] is closed.
    pub member_id: MemberId,
    /// Reason of why [`RpcConnection`] is closed.
    pub reason: ClosedReason,
}

/// Reasons of why [`RpcConnection`] may be closed.
#[derive(Debug)]
pub enum ClosedReason {
    /// [`RpcConnection`] was irrevocably closed.
    Closed,
    /// [`RpcConnection`] was lost, but may be reestablished.
    Lost,
}

#[cfg(test)]
pub mod test {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };

    use actix::{
        Actor, ActorContext, Addr, AsyncContext, Context, Handler, Message,
        System,
    };
    use futures::future::Future;

    use crate::{
        api::{
            client::rpc_connection::{
                ClosedReason, RpcConnection, RpcConnectionClosed,
                RpcConnectionEstablished,
            },
            control::MemberId,
            protocol::{Command, Event},
        },
        signalling::Room,
    };

    /// [`RpcConnection`] impl convenient for testing.
    #[derive(Debug, Clone)]
    pub struct TestConnection {
        pub member_id: MemberId,
        pub room: Addr<Room>,
        pub events: Arc<Mutex<Vec<String>>>,
        pub stopped: Arc<AtomicUsize>,
    }

    impl Actor for TestConnection {
        type Context = Context<Self>;

        fn started(&mut self, ctx: &mut Self::Context) {
            self.room
                .try_send(RpcConnectionEstablished {
                    member_id: self.member_id,
                    connection: Box::new(ctx.address()),
                })
                .unwrap();
        }

        fn stopped(&mut self, _ctx: &mut Self::Context) {
            self.stopped.fetch_add(1, Ordering::Relaxed);
            if self.stopped.load(Ordering::Relaxed) > 1 {
                System::current().stop()
            }
        }
    }

    #[derive(Message)]
    struct Close;

    impl Handler<Close> for TestConnection {
        type Result = ();

        fn handle(&mut self, _: Close, ctx: &mut Self::Context) {
            ctx.stop()
        }
    }

    impl Handler<Event> for TestConnection {
        type Result = ();

        fn handle(&mut self, event: Event, _ctx: &mut Self::Context) {
            let mut events = self.events.lock().unwrap();
            events.push(serde_json::to_string(&event).unwrap());
            match event {
                Event::PeerCreated {
                    peer_id,
                    sdp_offer,
                    tracks: _,
                } => {
                    match sdp_offer {
                        Some(_) => self.room.do_send(Command::MakeSdpAnswer {
                            peer_id,
                            sdp_answer: "responder_answer".into(),
                        }),
                        None => self.room.do_send(Command::MakeSdpOffer {
                            peer_id,
                            sdp_offer: "caller_offer".into(),
                        }),
                    }
                    self.room.do_send(Command::SetIceCandidate {
                        peer_id,
                        candidate: "ice_candidate".into(),
                    })
                }
                Event::IceCandidateDiscovered {
                    peer_id: _,
                    candidate: _,
                } => {
                    self.room.do_send(RpcConnectionClosed {
                        member_id: self.member_id,
                        reason: ClosedReason::Closed,
                    });
                }
                Event::PeersRemoved { peer_ids: _ } => {}
                Event::SdpAnswerMade {
                    peer_id: _,
                    sdp_answer: _,
                } => {}
            }
        }
    }

    impl RpcConnection for Addr<TestConnection> {
        fn close(&mut self) -> Box<dyn Future<Item = (), Error = ()>> {
            let fut = self.send(Close {}).map_err(|_| ());
            Box::new(fut)
        }

        fn send_event(
            &self,
            event: Event,
        ) -> Box<dyn Future<Item = (), Error = ()>> {
            let fut = self.send(event).map_err(|_| ());
            Box::new(fut)
        }
    }
}
