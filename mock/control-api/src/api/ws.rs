use std::{sync::Arc, time::Duration};

use actix::{
    Actor, ActorContext, AsyncContext, Handler, Message, StreamHandler,
};
use actix_web::{
    web::{Data, Path, Payload},
    Error, HttpRequest, HttpResponse,
};
use actix_web_actors::ws;
use serde::Serialize;
use serde_json::Value;

use crate::{
    api::{AppContext, Element, Subscribers},
    client::Fid,
    prelude::*,
};

/// Handles HTTP upgrade request, tries to perform handshake and establish
/// WebSocket connection.
///
/// # Errors
///
/// Errors if handshake fails for any underlying reason.
pub async fn create_ws(
    request: HttpRequest,
    path: Path<String>,
    state: Data<AppContext>,
    payload: Payload,
) -> Result<HttpResponse, Error> {
    ws::start(
        WsSession {
            room_id: path.into_inner(),
            subscribers: Arc::clone(&state.subscribers),
            last_ping_num: 0,
        },
        &request,
        payload,
    )
}

/// Notification that some mutating operation was performed to some `Room`.
#[derive(Clone, Debug, Message, Serialize)]
#[rtype(result = "()")]
pub struct Notification(Value);

/// [`Notification`] serialization helper.
#[derive(Serialize)]
#[serde(tag = "method")]
enum NotificationVariants<'a> {
    Created { fid: &'a str, element: &'a Element },
    Deleted { fid: &'a str },
}

impl Notification {
    /// Builds `method: Created` [`Notification`].
    pub fn created(fid: &Fid, element: &Element) -> Notification {
        Self(
            serde_json::to_value(NotificationVariants::Created {
                fid: fid.as_ref(),
                element,
            })
            .unwrap(),
        )
    }

    /// Builds `method: Deleted` [`Notification`].
    pub fn deleted(fid: &Fid) -> Notification {
        Self(
            serde_json::to_value(NotificationVariants::Deleted {
                fid: fid.as_ref(),
            })
            .unwrap(),
        )
    }
}

/// WebSocket connection with [`Notification`] subscriber.
#[derive(Default)]
struct WsSession {
    /// `Room` id that this `WsSession` is subscribed to.
    room_id: String,
    /// Map of subscribers to [`Notification`]s.
    subscribers: Subscribers,
    /// `Ping` messages counter.
    last_ping_num: u32,
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    /// Adds session to sessions map and schedules ping task.
    fn started(&mut self, ctx: &mut Self::Context) {
        let this = ctx.address().recipient();

        self.subscribers
            .lock()
            .unwrap()
            .entry(self.room_id.clone())
            .or_default()
            .push(this);

        ctx.run_interval(
            Duration::from_secs(10),
            |this: &mut WsSession, ctx| {
                this.last_ping_num += 1;
                ctx.ping(&this.last_ping_num.to_be_bytes());
            },
        );
    }

    /// Removes session from sessions map.
    fn stopped(&mut self, ctx: &mut Self::Context) {
        let recipient = ctx.address().recipient();
        if let Some(subs) =
            self.subscribers.lock().unwrap().get_mut(&self.room_id)
        {
            subs.retain(|sub| *sub != recipient)
        }
    }
}

impl Handler<Notification> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: Notification, ctx: &mut Self::Context) {
        ctx.text(serde_json::to_string(&msg).unwrap());
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(
        &mut self,
        msg: Result<ws::Message, ws::ProtocolError>,
        ctx: &mut Self::Context,
    ) {
        match msg {
            Ok(msg) => match msg {
                ws::Message::Ping(ping) => {
                    ctx.pong(&ping);
                }
                ws::Message::Close(reason) => {
                    ctx.close(reason);
                    ctx.stop();
                }
                ws::Message::Pong(_) => {}
                _ => error!("Unsupported client message: {:?}", msg),
            },
            Err(err) => {
                error!("Ws StreamHandler error {}", err);
            }
        };
    }
}
