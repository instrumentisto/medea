use std::rc::{Rc, Weak};

use crate::utils::task_spawner::{HasTaskHandlesStorage, TaskHandlesStorage};
use futures::{future, Future, Stream, StreamExt};
use wasm_bindgen_futures::spawn_local;
use std::cell::RefCell;

pub struct Component<S, C> {
    state: Rc<S>,
    ctx: C,
    task_handles: TaskHandlesStorage,
}

impl<S, C> Component<S, RefCell<Weak<C>>> {
    pub fn without_context(state: Rc<S>) -> Self {
        Self {
            state,
            ctx: RefCell::new(Weak::new()),
            task_handles: TaskHandlesStorage::new(),
        }
    }

    pub fn replace_context(&self, new_ctx: Weak<C>) {
        self.ctx.replace(new_ctx);
    }
}

impl<S, C: 'static> Component<S, Rc<C>> {
    pub fn new_component(state: Rc<S>, ctx: Rc<C>) -> Self {
        Self {
            state,
            ctx,
            task_handles: TaskHandlesStorage::new(),
        }
    }

    // TODO: temporary
    pub fn ctx(&self) -> Rc<C> {
        self.ctx.clone()
    }
}

impl<S, C: 'static + Clone> Component<S, C> {
    /// Spawns listener for the [`Observable`].
    ///
    /// You can stop all listeners tasks spawned by this function by calling
    /// [`TaskDisposer::dispose_tasks`]
    pub fn spawn_task<R, V, F, O>(&self, mut rx: R, handle: F)
        where
            F: Fn(C, V) -> O + 'static,
            R: Stream<Item = V> + Unpin + 'static,
            O: Future<Output = ()> + 'static,
    {
        let ctx = self.ctx.clone();
        let (fut, handle) = future::abortable(async move {
            while let Some(value) = rx.next().await {
                (handle)(ctx.clone(), value).await;
            }
        });
        spawn_local(async move {
            let _ = fut.await;
        });
        self.task_handles.register_handle(handle);
    }
}

impl<S, C> Component<S, C> {
    pub fn state(&self) -> &S {
        &self.state
    }
}

impl<S, C> HasTaskHandlesStorage for Component<S, C> {
    fn task_handles_storage(&self) -> &TaskHandlesStorage {
        &self.task_handles
    }
}
