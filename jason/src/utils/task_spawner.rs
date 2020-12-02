use std::{cell::RefCell, ops::Deref, rc::Rc};

use futures::{future, future::AbortHandle, Future, Stream, StreamExt as _};
use wasm_bindgen_futures::spawn_local;

/// Storage of the [`AbortHandle`]s used for aborting [`Observable`] listeners
/// tasks.
pub struct TaskHandlesStorage(RefCell<Vec<AbortHandle>>);

impl TaskHandlesStorage {
    pub fn new() -> Self {
        Self(RefCell::default())
    }

    pub fn register_handle(&self, handle: AbortHandle) {
        self.0.borrow_mut().push(handle);
    }

    /// Aborts all spawned [`Observable`] listeners tasks registered in this
    /// [`TaskHandlesStorage`].
    pub(self) fn dispose(&self) {
        let handles: Vec<_> = std::mem::take(&mut self.0.borrow_mut());
        for handle in handles {
            handle.abort();
        }
    }
}

/// Wrapper around disposable components which will call
/// [`TaskDisposer::dispose_tasks`] on [`Drop`].
#[derive(Debug)]
pub struct RootComponent<T: TaskDisposer>(Rc<T>);

impl<T> RootComponent<T>
where
    T: TaskDisposer,
{
    pub fn new(component: T) -> Self {
        Self(Rc::new(component))
    }

    /// Returns [`Rc`] to the underlying component.
    ///
    /// Returned component can be used in the [`Observable`] listeners spawned
    /// by [`ObservableSpawner::spawn_task`] and it wouldn't dispose component
    /// on [`Drop`].
    pub fn reference(&self) -> Rc<T> {
        Rc::clone(&self.0)
    }
}

impl<T> Drop for RootComponent<T>
where
    T: TaskDisposer,
{
    fn drop(&mut self) {
        self.0.dispose_tasks();
    }
}

impl<T> Deref for RootComponent<T>
where
    T: TaskDisposer,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub trait TaskDisposer: HasTaskHandlesStorage {
    /// Aborts all tasks spawned by [`ObservableSpawner::spawn_task`].
    fn dispose_tasks(&self) {
        self.task_handles_storage().dispose();
    }
}

pub trait HasTaskHandlesStorage {
    /// Returns [`TaskHandlesStorage`] of this component.
    ///
    /// In the returned [`TaskHandlesStorage`] all spawned listeners will be
    /// stored and with this [`TaskHandlesStorage`] all task will be stopped on
    /// [`TaskDisposer::dispose_tasks`] call.
    fn task_handles_storage(&self) -> &TaskHandlesStorage;
}

pub trait ObservableSpawner: HasTaskHandlesStorage {
    /// Spawns listener for the [`Observable`].
    ///
    /// You can stop all listeners tasks spawned by this function by calling
    /// [`TaskDisposer::dispose_tasks`]
    fn spawn_task<S, V, F, C, O>(&self, mut rx: S, ctx: C, handle: F)
    where
        F: Fn(Rc<C>, V) -> O + 'static,
        S: Stream<Item = V> + Unpin + 'static,
        O: Future<Output = ()> + 'static,
        C: 'static,
    {
        let (fut, handle) = future::abortable(async move {
            let ctx = Rc::new(ctx);
            while let Some(value) = rx.next().await {
                (handle)(Rc::clone(&ctx), value).await;
            }
        });
        spawn_local(async move {
            let _ = fut.await;
        });
        self.task_handles_storage().register_handle(handle);
    }
}

impl<T> TaskDisposer for T where T: HasTaskHandlesStorage {}

impl<T> ObservableSpawner for T where T: HasTaskHandlesStorage {}
