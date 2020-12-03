use std::rc::{Rc, Weak};

use crate::utils::task_spawner::{HasTaskHandlesStorage, TaskHandlesStorage};
use futures::{future, Future, Stream, StreamExt};
use std::cell::RefCell;
use wasm_bindgen_futures::spawn_local;

pub struct Component<S, C, G> {
    state: Rc<S>,
    ctx: C,
    global_ctx: Rc<G>,
    task_handles: TaskHandlesStorage,
}

impl<S, C, G> Component<S, RefCell<Weak<C>>, G> {
    pub fn without_context(state: Rc<S>, global_ctx: Rc<G>) -> Self {
        Self {
            state,
            ctx: RefCell::new(Weak::new()),
            global_ctx,
            task_handles: TaskHandlesStorage::new(),
        }
    }

    pub fn replace_context(&self, new_ctx: Weak<C>) {
        self.ctx.replace(new_ctx);
    }
}

impl<S, C: 'static, G> Component<S, Rc<C>, G> {
    pub fn new_component(state: Rc<S>, ctx: Rc<C>, global_ctx: Rc<G>) -> Self {
        Self {
            state,
            ctx,
            global_ctx,
            task_handles: TaskHandlesStorage::new(),
        }
    }

    // TODO: temporary
    pub fn ctx(&self) -> Rc<C> {
        self.ctx.clone()
    }
}

impl<S: 'static, C: 'static + Clone, G: 'static> Component<S, C, G> {
    /// Spawns listener for the [`Observable`].
    ///
    /// You can stop all listeners tasks spawned by this function by calling
    /// [`TaskDisposer::dispose_tasks`]
    pub fn spawn_task<R, V, F, O>(&self, mut rx: R, handle: F)
    where
        F: Fn(C, Rc<G>, Rc<S>, V) -> O + 'static,
        R: Stream<Item = V> + Unpin + 'static,
        O: Future<Output = ()> + 'static,
    {
        let ctx = self.ctx.clone();
        let global_ctx = Rc::clone(&self.global_ctx);
        let state = Rc::clone(&self.state);
        let (fut, handle) = future::abortable(async move {
            while let Some(value) = rx.next().await {
                (handle)(
                    ctx.clone(),
                    Rc::clone(&global_ctx),
                    Rc::clone(&state),
                    value,
                )
                .await;
            }
        });
        spawn_local(async move {
            let _ = fut.await;
        });
        self.task_handles.register_handle(handle);
    }
}

impl<S, C, G> Component<S, C, G> {
    pub fn state(&self) -> &S {
        &self.state
    }
}

impl<S, C, G> HasTaskHandlesStorage for Component<S, C, G> {
    fn task_handles_storage(&self) -> &TaskHandlesStorage {
        &self.task_handles
    }
}
