use std::{cell::RefCell, rc::Rc};

use derive_more::{Display, From};
use futures::{
    channel::mpsc,
    future::{self, AbortHandle},
    stream::LocalBoxStream,
    StreamExt as _,
};
use medea_client_api_proto::{ClientMsg, ServerMsg};
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;

use crate::{
    rpc::{RpcTransport, TransportError},
    utils::{console_error, resolve_after, JsCaused, JsDuration, JsError},
};

/// Errors that may occur in [`Heartbeat`].
#[derive(Debug, Display, From, JsCaused)]
pub struct HeartbeatError(TransportError);

type HeartbeatResult<T> = std::result::Result<T, Traced<HeartbeatError>>;

/// Wrapper around [`AbortHandle`] which will abort [`Future`] on [`Drop`].
#[derive(Debug, From)]
struct Abort(AbortHandle);

impl Drop for Abort {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Idle timeout of [`RpcClient`].
#[derive(Debug, Copy, Clone)]
pub struct IdleTimeout(pub JsDuration);

/// Ping interval of [`RpcClient`].
#[derive(Debug, Copy, Clone)]
pub struct PingInterval(pub JsDuration);

struct Inner {
    /// [`RpcTransport`] which heartbeats.
    transport: Option<Rc<dyn RpcTransport>>,

    /// IDLE timeout of [`RpcClient`].
    idle_timeout: IdleTimeout,

    /// Ping interval of [`RpcClient`].
    ping_interval: PingInterval,

    /// [`Abort`] for [`Future`] which sends [`ClientMsg::Pong`] on
    /// [`ServerMsg::Ping`].
    handle_ping_task: Option<Abort>,

    /// [`Abort`] for idle resolver task.
    idle_watchdog_task: Option<Abort>,

    /// Number of last received [`ServerMsg::Ping`].
    last_ping_num: u64,

    /// [`mpsc::UnboundedSender`]s for a [`Heartbeat::on_idle`].
    on_idle_subs: Vec<mpsc::UnboundedSender<()>>,
}

impl Inner {
    /// Sends [`ClientMsg::Pong`] to a server.
    ///
    /// If some error happen then this will be printed with [`console_error`].
    fn send_pong(&self, n: u64) {
        self.transport
            .as_ref()
            .ok_or_else(|| {
                let e = tracerr::new!(
                    "RpcTransport from Heartbeat unexpectedly gone."
                );
                format!("{}\n{}", e, e.trace())
            })
            .and_then(|t| {
                t.send(&ClientMsg::Pong(n))
                    .map_err(|e| format!("{}\n{}", e, e.trace()))
            })
            .map_err(console_error)
            .ok();
    }
}

/// Service for sending/receiving ping pongs between the client and server.
pub struct Heartbeat(Rc<RefCell<Inner>>);

impl Heartbeat {
    // TODO: i dont like default-random values for IdleTimeout and PingInterval.
    //       if RpcSettingsUpdated is sent each time new ws-connection is
    //       established, then it makes sense to resolve RpcTransport
    //       creation on this message receival, this way we will have latest
    //       rpc-settings known when RpcTransport will be connected and we
    //       can pass those settings to start() function. Also, it seems
    //       that RpcSettingsUpdated should be moved from Events to ServerMsg.

    /// Creates new [`Heartbeat`] with provided config.
    pub fn new(idle_timeout: IdleTimeout, ping_interval: PingInterval) -> Self {
        Self(Rc::new(RefCell::new(Inner {
            idle_timeout,
            transport: None,
            handle_ping_task: None,
            on_idle_subs: Vec::new(),
            idle_watchdog_task: None,
            ping_interval,
            last_ping_num: 0,
        })))
    }

    /// Start heartbeats for provided [`RpcTransport`].
    pub fn start(
        &self,
        transport: Rc<dyn RpcTransport>,
    ) -> HeartbeatResult<()> {
        let weak_this = Rc::downgrade(&self.0);
        let mut on_message_stream = transport
            .on_message()
            .map_err(tracerr::map_from_and_wrap!())?;
        self.0.borrow_mut().transport = Some(transport);
        self.reset_idle_resolver();
        let (fut, pong_abort) = future::abortable(async move {
            while let Some((this, msg)) =
                on_message_stream.next().await.and_then(|msg| {
                    weak_this.upgrade().map(move |t| (Self(t), msg))
                })
            {
                this.reset_idle_resolver();

                if let ServerMsg::Ping(num) = msg {
                    this.0.borrow_mut().last_ping_num = num;
                    this.0.borrow().send_pong(num);
                }
            }
        });
        spawn_local(async move {
            fut.await.ok();
        });
        self.0.borrow_mut().handle_ping_task = Some(pong_abort.into());

        Ok(())
    }

    /// Stops [`Heartbeat`].
    pub fn stop(&self) {
        self.0.borrow_mut().transport.take();
        self.0.borrow_mut().handle_ping_task.take();
        self.0.borrow_mut().idle_watchdog_task.take();
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

    /// Returns [`LocalBoxStream`] to which will be sent unit message when
    /// [`Heartbeat`] considers that [`RpcTransport`] is IDLE.
    pub fn on_idle(&self) -> LocalBoxStream<'static, ()> {
        let (on_idle_tx, on_idle_rx) = mpsc::unbounded();
        self.0.borrow_mut().on_idle_subs.push(on_idle_tx);

        Box::pin(on_idle_rx)
    }

    /// Aborts idle resolver and sets new one.
    fn reset_idle_resolver(&self) {
        // TODO: perhaps, using window.set_interval with some ping_at will be
        //       more convenient?
        self.0.borrow_mut().idle_watchdog_task.take();

        let weak_this = Rc::downgrade(&self.0);
        let (idle_watchdog, idle_watchdog_handle) =
            future::abortable(async move {
                if let Some(this) = weak_this.upgrade() {
                    // TODO: perhaps idle_timeout / 2?
                    let wait_for_ping = this.borrow().ping_interval.0 * 2;
                    resolve_after(wait_for_ping).await;

                    let last_ping_num = this.borrow().last_ping_num;
                    this.borrow().send_pong(last_ping_num + 1);

                    let idle_timeout = this.borrow().idle_timeout;
                    resolve_after(idle_timeout.0 - wait_for_ping).await;
                    this.borrow_mut()
                        .on_idle_subs
                        .retain(|sub| !sub.is_closed());
                    for sub in &this.borrow().on_idle_subs {
                        if sub.unbounded_send(()).is_err() {
                            console_error(
                                "Heartbeat::on_idle subscriber unexpectedly \
                                 gone.",
                            );
                        }
                    }
                }
            });

        spawn_local(async move {
            idle_watchdog.await.ok();
        });

        self.0.borrow_mut().idle_watchdog_task =
            Some(idle_watchdog_handle.into());
    }
}
