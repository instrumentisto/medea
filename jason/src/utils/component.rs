use std::rc::Rc;

use crate::utils::task_spawner::{TaskHandlesStorage, HasTaskHandlesStorage};
use futures::{Stream, Future, StreamExt};
use futures::future;
use wasm_bindgen_futures::spawn_local;

pub struct Component<S, C> {
    state: Rc<S>,
    ctx: Rc<C>,
    task_handles: TaskHandlesStorage,
}

impl<S, C: 'static> Component<S, C> {
    pub fn new_component(state: Rc<S>, ctx: C) -> Self {
        Self {
            state,
            ctx: Rc::new(ctx),
            task_handles: TaskHandlesStorage::new(),
        }
    }

    pub fn state(&self) -> &S {
        &self.state
    }

    // TODO: temporary
    pub fn ctx(&self) -> Rc<C> {
        self.ctx.clone()
    }

    /// Spawns listener for the [`Observable`].
    ///
    /// You can stop all listeners tasks spawned by this function by calling
    /// [`TaskDisposer::dispose_tasks`]
    pub fn spawn_task<R, V, F, O>(&self, mut rx: R, handle: F)
        where
            F: Fn(Rc<C>, V) -> O + 'static,
            R: Stream<Item = V> + Unpin + 'static,
            O: Future<Output = ()> + 'static,
    {
        let ctx = Rc::clone(&self.ctx);
        let (fut, handle) = future::abortable(async move {
            while let Some(value) = rx.next().await {
                (handle)(Rc::clone(&ctx), value).await;
            }
        });
        spawn_local(async move {
            let _ = fut.await;
        });
        self.task_handles.register_handle(handle);
    }
}

impl<S, C> HasTaskHandlesStorage for Component<S, C> {
    fn task_handles_storage(&self) -> &TaskHandlesStorage {
        &self.task_handles
    }
}
