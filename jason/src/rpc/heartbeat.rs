//! Implementation of connection loss detection through Ping/Pong mechanism.

use std::{cell::RefCell, rc::Rc, time::Duration};

use derive_more::{Display, From, Mul};
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
#[derive(Debug, Copy, Clone, Mul)]
pub struct PingInterval(pub JsDuration);

struct Inner {
    /// [`RpcTransport`] which heartbeats.
    transport: Option<Rc<dyn RpcTransport>>,

    /// Idle timeout of [`RpcClient`].
    idle_timeout: IdleTimeout,

    /// Ping interval of [`RpcClient`].
    ping_interval: PingInterval,

    /// [`Abort`] for [`Future`] which sends [`ClientMsg::Pong`] on
    /// [`ServerMsg::Ping`].
    handle_ping_task: Option<Abort>,

    /// [`Abort`] for idle watchdog.
    idle_watchdog_task: Option<Abort>,

    /// Number of last received [`ServerMsg::Ping`].
    last_ping_num: u64,

    /// [`mpsc::UnboundedSender`]s for a [`Heartbeat::on_idle`].
    on_idle_subs: Vec<mpsc::UnboundedSender<()>>,
}

impl Inner {
    /// Sends [`ClientMsg::Pong`] to a server.
    ///
    /// If some error happen then it will be printed with [`console_error`].
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

/// Service for detecting connection loss through ping/pong mechanism.
pub struct Heartbeat(Rc<RefCell<Inner>>);

impl Heartbeat {
    /// Creates new [`Heartbeat`].
    ///
    /// By default `idle_timeout` will be set to 10 seconds and `ping_interval`
    /// to 3 seconds. But this default values wouldn't be used anywhere. This
    /// defaults is used only to avoid useless [`Option`] usage.
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Inner {
            idle_timeout: IdleTimeout(Duration::from_secs(10).into()),
            ping_interval: PingInterval(Duration::from_secs(3).into()),
            transport: None,
            handle_ping_task: None,
            on_idle_subs: Vec::new(),
            idle_watchdog_task: None,
            last_ping_num: 0,
        })))
    }

    /// Start heartbeating for provided [`RpcTransport`] with provided
    /// `idle_timeout` and `ping_interval`.
    ///
    /// If heartbeating is already started then old settings, idle watchdog and
    /// ponger will be cancelled.
    pub fn start(
        &self,
        idle_timeout: IdleTimeout,
        ping_interval: PingInterval,
        transport: Rc<dyn RpcTransport>,
    ) -> Result<(), Traced<HeartbeatError>> {
        let mut on_message_stream = transport
            .on_message()
            .map_err(tracerr::map_from_and_wrap!())?;
        self.0.borrow_mut().transport = Some(transport);
        self.0.borrow_mut().ping_interval = ping_interval;
        self.0.borrow_mut().idle_timeout = idle_timeout;

        self.reset_idle_watchdog();

        let weak_this = Rc::downgrade(&self.0);
        let (fut, pong_abort) = future::abortable(async move {
            while let Some((this, msg)) =
                on_message_stream.next().await.and_then(|msg| {
                    weak_this.upgrade().map(move |t| (Self(t), msg))
                })
            {
                this.reset_idle_watchdog();

                if let ServerMsg::Ping(num) = msg {
                    this.0.borrow_mut().last_ping_num = num;
                    this.0.borrow().send_pong(num);
                }
            }
        });
        spawn_local(async move {
            // Ignore this Abort error because aborting is normal behavior of
            // this Future.
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

    /// Updates [`Heartbeat`] settings.
    pub fn update_settings(
        &self,
        idle_timeout: IdleTimeout,
        ping_interval: PingInterval,
    ) {
        self.0.borrow_mut().idle_timeout = idle_timeout;
        self.0.borrow_mut().ping_interval = ping_interval;
    }

    /// Returns [`LocalBoxStream`] to which will be sent `()` when
    /// [`Heartbeat`] considers that [`RpcTransport`] is idle.
    pub fn on_idle(&self) -> LocalBoxStream<'static, ()> {
        let (on_idle_tx, on_idle_rx) = mpsc::unbounded();
        self.0.borrow_mut().on_idle_subs.push(on_idle_tx);

        Box::pin(on_idle_rx)
    }

    /// Resets `idle_watchdog` task and sets new one.
    ///
    /// This watchdog is responsible for throwing [`Heartbeat::on_idle`] when
    /// [`ServerMsg`] isn't received within `idle_timeout`.
    ///
    /// Also this watchdog will try to send [`ClientMsg::Pong`] if
    /// [`ServerMsg::Ping`] wasn't received within `ping_interval * 2`.
    fn reset_idle_watchdog(&self) {
        self.0.borrow_mut().idle_watchdog_task.take();

        let weak_this = Rc::downgrade(&self.0);
        let (idle_watchdog, idle_watchdog_handle) =
            future::abortable(async move {
                let this = if let Some(this) = weak_this.upgrade() {
                    this
                } else {
                    return;
                };
                let wait_for_ping = this.borrow().ping_interval * 2;
                resolve_after(wait_for_ping.0).await;

                let last_ping_num = this.borrow().last_ping_num;
                this.borrow().send_pong(last_ping_num + 1);

                let idle_timeout = this.borrow().idle_timeout;
                resolve_after(idle_timeout.0 - wait_for_ping.0).await;
                this.borrow_mut()
                    .on_idle_subs
                    .retain(|sub| !sub.is_closed());
                this.borrow()
                    .on_idle_subs
                    .iter()
                    .filter_map(|sub| sub.unbounded_send(()).err())
                    .for_each(|err| {
                        console_error(format!(
                            "Heartbeat::on_idle subscriber unexpectedly gone. \
                             {:?}",
                            err
                        ))
                    });
            });

        spawn_local(async move {
            // Ignore this Abort error because aborting is normal behavior of
            // watchdog.
            idle_watchdog.await.ok();
        });

        self.0.borrow_mut().idle_watchdog_task =
            Some(idle_watchdog_handle.into());
    }
}
