use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use futures::{future, Future, Stream, StreamExt};
use wasm_bindgen_futures::spawn_local;

use crate::utils::{task_spawner::TaskHandlesStorage, JasonError};
use bitflags::_core::ops::Deref;

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

    pub fn ctx(&self) -> Rc<C> {
        self.ctx.clone()
    }
}

impl<S: 'static, C: 'static + Clone, G: 'static> Component<S, C, G> {
    /// Spawns listener for the [`Observable`].
    ///
    /// You can stop all listeners tasks spawned by this function by calling
    /// [`TaskDisposer::dispose_tasks`]
    pub fn spawn_observer<R, V, F, O, E>(&self, mut rx: R, handle: F)
    where
        F: Fn(C, Rc<G>, Rc<S>, V) -> O + 'static,
        R: Stream<Item = V> + Unpin + 'static,
        O: Future<Output = Result<(), E>> + 'static,
        E: Into<JasonError>,
    {
        let ctx = self.ctx.clone();
        let global_ctx = Rc::clone(&self.global_ctx);
        let state = Rc::clone(&self.state);
        let (fut, handle) = future::abortable(async move {
            while let Some(value) = rx.next().await {
                if let Err(e) = (handle)(
                    ctx.clone(),
                    Rc::clone(&global_ctx),
                    Rc::clone(&state),
                    value,
                )
                .await
                {
                    Into::<JasonError>::into(e).print();
                }
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

impl<S, C, G> Drop for Component<S, C, G> {
    fn drop(&mut self) {
        self.task_handles.dispose();
    }
}

impl<S, C, G> Deref for Component<S, C, G> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.ctx
    }
}
