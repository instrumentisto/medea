use crate::*;
use actix_web::{fs, http, middleware, server, ws, App, Error, HttpRequest, HttpResponse};
use api::control::member::{Member, MemberRepository};
use im::hashmap::HashMap;
use std::time::{Duration, Instant};

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// do websocket handshake and start `WsSessions` actor
fn ws_index(r: &HttpRequest) -> Result<HttpResponse, Error> {
    ws::start(r, WsSessions::new())
}

fn index(r: &HttpRequest) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().finish())
}

/// websocket connection is long running connection, it easier
/// to handle with an actor
struct WsSessions {
    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    hb: Instant,
}

impl Actor for WsSessions {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start. We start the heartbeat process here.
    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
    }
}

/// Handler for `ws::Message`
impl StreamHandler<ws::Message, ws::ProtocolError> for WsSessions {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        // process websocket messages
        println!("WS: {:?}", msg);
        match msg {
            ws::Message::Ping(msg) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.hb = Instant::now();
            }
            ws::Message::Text(text) => ctx.text(text),
            ws::Message::Binary(bin) => ctx.binary(bin),
            ws::Message::Close(_) => {
                ctx.stop();
            }
        }
    }
}

impl WsSessions {
    fn new() -> Self {
        Self { hb: Instant::now() }
    }

    /// helper method that sends ping to client every second.
    ///
    /// also this method checks heartbeats from client
    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                println!("Websocket Client heartbeat failed, disconnecting!");

                // stop actor
                ctx.stop();

                // don't try to send a ping
                return;
            }

            ctx.ping("");
        });
    }
}

/// State with repository address
struct AppState {
    members_repo: Addr<MemberRepository>,
}

pub fn run() {
    server::new(|| {
        with_state(AppState {
            members_repo: addr.clone(),
        })
        // enable logger
        .middleware(middleware::Logger::default())
        // websocket route
        .resource("/ws/", |r| r.method(http::Method::GET).f(ws_index))
        .resource("/get/", |r| r.method(http::Method::GET).f(index))
    })
    // start http server on 127.0.0.1:8080
    .bind("127.0.0.1:8080")
    .unwrap()
    .start();

    println!("Started http server: 127.0.0.1:8080");
    trace!("Started http server: 127.0.0.1:8080");
}

fn init_repo<A: Actor>() -> Addr<A> {
    let mut members = HashMap::new();
    members.insert(
        1,
        Member {
            id: 1,
            credentials: "user1_credentials".to_owned(),
        },
    );
    members.insert(
        2,
        Member {
            id: 2,
            credentials: "user2_credentials".to_owned(),
        },
    );

    info!("Repository created");

    Arbiter::builder().start(move |_| MemberRepository { members })
}
