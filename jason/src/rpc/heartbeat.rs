use std::{cell::RefCell, rc::Rc};

use derive_more::{Display, From};
use futures::{
    channel::mpsc,
    future::{self, AbortHandle, Abortable},
    stream::LocalBoxStream,
    StreamExt as _,
};
use js_sys::Date;
use medea_client_api_proto::{ClientMsg, ServerMsg};
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::spawn_local;

use crate::{
    rpc::{RpcTransport, TransportError},
    utils::{
        console_error, resolve_after, window, IntervalHandle, JsCaused, JsError,
    },
};

#[derive(Clone, Copy, Debug, Display, From)]
pub struct Timestamp(pub u64);

impl Timestamp {
    pub fn now() -> Self {
        Self(Date::now() as u64)
    }
}

/// Errors that may occur in [`Heartbeat`].
#[derive(Debug, Display, From, JsCaused)]
pub enum HeartbeatError {
    Transport(#[js(cause)] TransportError),
}

type Result<T> = std::result::Result<T, Traced<HeartbeatError>>;

#[derive(Debug, From)]
struct Abort(AbortHandle);

impl Drop for Abort {
    fn drop(&mut self) {
        self.0.abort();
    }
}

struct Inner {
    idle_timeout: Timestamp,
    transport: Option<Rc<dyn RpcTransport>>,
    last_activity: Timestamp,
    pong_task_abort: Option<Abort>,
    idle_sender: Option<mpsc::UnboundedSender<()>>,
    idle_resolver_abort: Option<Abort>,
}

pub struct Heartbeat(Rc<RefCell<Inner>>);

impl Heartbeat {
    pub fn new(idle_timeout: Timestamp) -> Self {
        Self(Rc::new(RefCell::new(Inner {
            idle_timeout,
            transport: None,
            last_activity: Timestamp::now(),
            pong_task_abort: None,
            idle_sender: None,
            idle_resolver_abort: None,
        })))
    }

    fn update_idle_resolver(&self) {
        self.0.borrow_mut().idle_resolver_abort.take();

        let weak_this = Rc::downgrade(&self.0);
        let (idle_resolver, idle_resolver_abort) =
            future::abortable(async move {
                // FIXME (evdokimovs): use ping interval from server
                if let Some(this) = weak_this.upgrade() {
                    let idle_timeout = this.borrow().idle_timeout;
                    // FIXME (evdokimovs): u64 as i32 look very bad
                    resolve_after(idle_timeout.0 as i32).await.unwrap();
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

    pub fn start(&self, transport: Rc<dyn RpcTransport>) -> Result<()> {
        let weak_this = Rc::downgrade(&self.0);
        let mut on_message_stream = transport
            .on_message()
            .map_err(tracerr::map_from_and_wrap!())?;
        self.update_idle_resolver();
        let (fut, pong_abort) = future::abortable(async move {
            while let Some(msg) = on_message_stream.next().await {
                if let Some(this) = weak_this.upgrade().map(Heartbeat) {
                    this.update_idle_resolver();
                    this.0.borrow_mut().last_activity = Timestamp::now();

                    if let ServerMsg::Ping(num) = msg {
                        if let Some(transport) =
                            &mut this.0.borrow_mut().transport
                        {
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

    pub fn stop(&self) {
        self.0.borrow_mut().transport.take();
        self.0.borrow_mut().pong_task_abort.take();
        self.0.borrow_mut().idle_resolver_abort.take();
    }

    pub fn on_idle(&self) -> LocalBoxStream<'_, ()> {
        let (on_idle_tx, on_idle_rx) = mpsc::unbounded();
        self.0.borrow_mut().idle_sender = Some(on_idle_tx);

        Box::pin(on_idle_rx)
    }

    pub fn get_last_activity(&self) -> Timestamp {
        self.0.borrow().last_activity
    }
}
