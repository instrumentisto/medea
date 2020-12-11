use std::{ops::Deref, rc::Rc};

use futures::{future, Future, Stream, StreamExt};
use wasm_bindgen_futures::spawn_local;

use crate::utils::JasonError;
use futures::future::AbortHandle;
use std::cell::RefCell;

#[macro_export]
macro_rules! spawn_component {
    ($component:ty, $state:expr, $ctx:expr, $global_ctx:expr $(,)*) => {{
        let component = <$component>::new($state, $ctx, $global_ctx);
        component.spawn();
        component
    }};
}

/// Storage of the [`AbortHandle`]s used for aborting [`Observable`] listeners
/// tasks.
#[derive(Default)]
struct WatchersStorage(RefCell<Vec<AbortHandle>>);

impl WatchersStorage {
    fn register_handle(&self, handle: AbortHandle) {
        self.0.borrow_mut().push(handle);
    }

    /// Aborts all spawned [`Observable`] listeners tasks registered in this
    /// [`TaskHandlesStorage`].
    fn dispose(&self) {
        let handles: Vec<_> = std::mem::take(&mut self.0.borrow_mut());
        for handle in handles {
            handle.abort();
        }
    }
}

pub struct Component<S, C, G> {
    state: Rc<S>,
    ctx: Rc<C>,
    global_ctx: Rc<G>,
    watchers_store: WatchersStorage,
}

impl<S, C: 'static, G> Component<S, C, G> {
    pub fn new(state: Rc<S>, ctx: Rc<C>, global_ctx: Rc<G>) -> Self {
        Self {
            state,
            ctx,
            global_ctx,
            watchers_store: WatchersStorage::default(),
        }
    }

    pub fn ctx(&self) -> Rc<C> {
        self.ctx.clone()
    }
}

impl<S: 'static, C: 'static, G: 'static> Component<S, C, G> {
    /// Spawns listener for the [`Observable`].
    ///
    /// You can stop all listeners tasks spawned by this function by calling
    /// [`TaskDisposer::dispose_tasks`]
    pub fn spawn_watcher<R, V, F, O, E>(&self, mut rx: R, handle: F)
    where
        F: Fn(Rc<C>, Rc<G>, Rc<S>, V) -> O + 'static,
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
        self.watchers_store.register_handle(handle);
    }
}

impl<S, C, G> Component<S, C, G> {
    pub fn state(&self) -> &S {
        &self.state
    }
}

impl<S, C, G> Drop for Component<S, C, G> {
    fn drop(&mut self) {
        self.watchers_store.dispose();
    }
}

impl<S, C, G> Deref for Component<S, C, G> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.ctx
    }
}
