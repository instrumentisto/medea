use std::{cell::RefCell, rc::Rc};

use derive_more::{Add, Display, From, Mul, Sub};
use futures::{
    channel::mpsc,
    future::{self, AbortHandle},
    stream::LocalBoxStream,
    StreamExt as _,
};
use js_sys::Date;
use medea_client_api_proto::{ClientMsg, ServerMsg};
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;

use crate::{
    rpc::{RpcTransport, TransportError},
    utils::{console_error, resolve_after, JsCaused, JsDuration, JsError},
};

/// Errors that may occur in [`Heartbeat`].
#[derive(Debug, Display, From, JsCaused)]
pub enum HeartbeatError {
    Transport(#[js(cause)] TransportError),
}

type Result<T> = std::result::Result<T, Traced<HeartbeatError>>;

/// Just wrapper around [`AbortHandle`] which will abort [`Future`] on [`Drop`].
#[derive(Debug, From)]
struct Abort(AbortHandle);

impl Drop for Abort {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// IDLE timeout of [`RpcClient`].
#[derive(Debug, Copy, Clone)]
pub struct IdleTimeout(pub JsDuration);

/// Ping interval of [`RpcClient`].
#[derive(Debug, Copy, Clone)]
pub struct PingInterval(pub JsDuration);

struct Inner {
    /// [`RpcTransport`] which heartbeats.
    transport: Option<Rc<dyn RpcTransport>>,

    /// [`Abort`] for [`Future`] which sends [`ClientMsg::Pong`] on
    /// [`ServerMsg::Ping`].
    pong_task_abort: Option<Abort>,

    /// Sender for [`Heartbeat::on_idle`].
    idle_sender: Option<mpsc::UnboundedSender<()>>,

    /// [`Abort`] for IDLE resolved task.
    idle_resolver_abort: Option<Abort>,

    /// Number of last received [`ServerMsg::Ping`].
    last_ping_num: u64,

    /// IDLE timeout of [`RpcClient`].
    idle_timeout: IdleTimeout,

    /// Ping interval of [`RpcClient`].
    ping_interval: PingInterval,
}

/// Service for ping pongs between client and server.
pub struct Heartbeat(Rc<RefCell<Inner>>);

impl Heartbeat {
    /// Creates new [`Heartbeat`] with provided config.
    pub fn new(idle_timeout: IdleTimeout, ping_interval: PingInterval) -> Self {
        Self(Rc::new(RefCell::new(Inner {
            idle_timeout,
            transport: None,
            pong_task_abort: None,
            idle_sender: None,
            idle_resolver_abort: None,
            ping_interval,
            last_ping_num: 1,
        })))
    }

    /// Aborts IDLE resolver and sets new one.
    fn update_idle_resolver(&self) {
        self.0.borrow_mut().idle_resolver_abort.take();

        let weak_this = Rc::downgrade(&self.0);
        let (idle_resolver, idle_resolver_abort) =
            future::abortable(async move {
                if let Some(this) = weak_this.upgrade() {
                    let wait_for_ping = this.borrow().ping_interval.0 * 2;
                    resolve_after(wait_for_ping).await.unwrap();
                    let last_ping_num = this.borrow().last_ping_num;
                    if let Some(transport) = &this.borrow().transport {
                        if let Err(e) =
                            transport.send(&ClientMsg::Pong(last_ping_num))
                        {
                            console_error(e.to_string());
                        }
                    } else {
                        console_error(
                            "RpcTransport from Heartbeat unexpectedly gone.",
                        );
                    }

                    let idle_timeout = this.borrow().idle_timeout;
                    resolve_after(idle_timeout.0 - wait_for_ping)
                        .await
                        .unwrap();
                    if let Some(idle_sender) = &this.borrow().idle_sender {
                        idle_sender.unbounded_send(());
                    }
                }
            });

        spawn_local(async move {
            idle_resolver.await;
        });

        self.0.borrow_mut().idle_resolver_abort =
            Some(idle_resolver_abort.into());
    }

    /// Start heartbeats for provided [`RpcTransport`].
    pub fn start(&self, transport: Rc<dyn RpcTransport>) -> Result<()> {
        let weak_this = Rc::downgrade(&self.0);
        let mut on_message_stream = transport
            .on_message()
            .map_err(tracerr::map_from_and_wrap!())?;
        self.0.borrow_mut().transport = Some(transport);
        self.update_idle_resolver();
        let (fut, pong_abort) = future::abortable(async move {
            while let Some(msg) = on_message_stream.next().await {
                if let Some(this) = weak_this.upgrade().map(Heartbeat) {
                    this.update_idle_resolver();

                    if let ServerMsg::Ping(num) = msg {
                        let last_ping_num = this.0.borrow().last_ping_num;
                        if last_ping_num == num {
                            continue;
                        }
                        this.0.borrow_mut().last_ping_num = num;
                        if let Some(transport) = &this.0.borrow().transport {
                            if let Err(e) =
                                transport.send(&ClientMsg::Pong(num))
                            {
                                console_error(e.to_string());
                            }
                        } else {
                            console_error(
                                "RpcTransport from Heartbeat unexpectedly \
                                 gone.",
                            );
                        }
                    }
                } else {
                    break;
                }
            }
        });
        spawn_local(async move {
            fut.await;
        });
        self.0.borrow_mut().pong_task_abort = Some(pong_abort.into());

        Ok(())
    }

    /// Stops heartbeat.
    pub fn stop(&self) {
        self.0.borrow_mut().transport.take();
        self.0.borrow_mut().pong_task_abort.take();
        self.0.borrow_mut().idle_resolver_abort.take();
    }

    /// Update [`RpcTransport`] settings.
    pub fn update_settings(
        &self,
        idle_timeout: IdleTimeout,
        ping_interval: PingInterval,
    ) {
        self.0.borrow_mut().idle_timeout = idle_timeout;
        self.0.borrow_mut().ping_interval = ping_interval;
    }

    /// Returns [`LocalBoxStream`] to which will be sent event when
    /// [`Heartbeat`] considers that [`RpcTransport`] is IDLE.
    pub fn on_idle(&self) -> LocalBoxStream<'static, ()> {
        let (on_idle_tx, on_idle_rx) = mpsc::unbounded();
        self.0.borrow_mut().idle_sender = Some(on_idle_tx);

        Box::pin(on_idle_rx)
    }
}
