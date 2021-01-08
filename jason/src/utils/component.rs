//! Implementation of the [`Component`].

use std::rc::Rc;

use derive_more::Deref;
use futures::{future, Future, Stream, StreamExt};
use wasm_bindgen_futures::spawn_local;

use crate::utils::{JasonError, TaskHandle};

/// Abstraction for the all states of the [`Component`].
pub trait ComponentState<C>: Sized {
    /// Spawns all watchers required for this [`ComponentState`].
    fn spawn_watchers(&self, watchers_spawner: &mut WatchersSpawner<Self, C>);
}

/// Base for all components of this app.
///
/// Spawns all watchers on [`Component::new`] function call.
///
/// Will stop all spawned with a [`Component::new`] watchers on [`Drop`].
///
/// Can be dereferenced to the [`Component`]'s context.
#[derive(Deref)]
pub struct Component<S, C> {
    #[deref]
    ctx: Rc<C>,
    state: Rc<S>,
    _spawned_watchers: Vec<TaskHandle>,
}

impl<S, C> Component<S, C> {
    /// Returns [`Rc`] to the context of this [`Component`].
    #[inline]
    pub fn ctx(&self) -> Rc<C> {
        self.ctx.clone()
    }

    /// Returns reference to the state of this [`Component`]
    #[inline]
    pub fn state(&self) -> &S {
        &self.state
    }
}

impl<S: ComponentState<C> + 'static, C: 'static> Component<S, C> {
    /// Returns new [`Component`] with a provided context and state.
    ///
    /// Spawns all watchers of this [`Component`].
    pub fn new(ctx: Rc<C>, state: Rc<S>) -> Self {
        let mut watchers_spawner =
            WatchersSpawner::new(Rc::clone(&state), Rc::clone(&ctx));
        state.spawn_watchers(&mut watchers_spawner);

        Self {
            state,
            ctx,
            _spawned_watchers: watchers_spawner.finish(),
        }
    }
}

/// Spawner for the [`Component`]'s watchers.
pub struct WatchersSpawner<S, C> {
    state: Rc<S>,
    ctx: Rc<C>,
    spawned_watchers: Vec<TaskHandle>,
}

impl<S: 'static, C: 'static> WatchersSpawner<S, C> {
    /// Creates new [`WatchersSpawner`] for the provided context and state.
    fn new(state: Rc<S>, ctx: Rc<C>) -> Self {
        Self {
            state,
            ctx,
            spawned_watchers: Vec::new(),
        }
    }

    /// Returns [`TaskHandle`] for the watchers spawned by this
    /// [`WatchersSpawner`].
    fn finish(self) -> Vec<TaskHandle> {
        self.spawned_watchers
    }

    // /// Spawns watchers for the provided [`Stream`].
    /// If watcher returns error then this error will be converted to the
    /// [`JasonError`] and printed with a [`JasonError::print`].
    ///
    /// You can stop all listeners tasks spawned by this function by
    /// [`Component`] drop.
    pub fn spawn<R, V, F, O, E>(&mut self, mut rx: R, handle: F)
    where
        F: Fn(Rc<C>, Rc<S>, V) -> O + 'static,
        R: Stream<Item = V> + Unpin + 'static,
        O: Future<Output = Result<(), E>> + 'static,
        E: Into<JasonError>,
    {
        let ctx = Rc::clone(&self.ctx);
        let state = Rc::clone(&self.state);
        let (fut, handle) = future::abortable(async move {
            while let Some(value) = rx.next().await {
                if let Err(e) =
                    (handle)(Rc::clone(&ctx), Rc::clone(&state), value).await
                {
                    Into::<JasonError>::into(e).print();
                }
            }
        });
        spawn_local(async move {
            let _ = fut.await;
        });
        self.spawned_watchers.push(handle.into());
    }
}
