//! Implementation of the [`Component`].

use std::rc::Rc;

use derive_more::Deref;
use futures::{future, Future, FutureExt as _, Stream, StreamExt as _};
use wasm_bindgen_futures::spawn_local;

use crate::utils::{JasonError, TaskHandle};

/// Component is a base that helps managing reactive components.
///
/// It consists of two parts: state and object. Object is listening to its state
/// changes and updates accordingly, so all mutations are meant to be applied to
/// the state.
#[derive(Deref)]
pub struct Component<S, O> {
    #[deref]
    obj: Rc<O>,
    state: Rc<S>,
    _spawned_watchers: Vec<TaskHandle>,
}

impl<S, O> Component<S, O> {
    /// Returns [`Rc`] to the object managed by this [`Component`].
    #[inline]
    #[must_use]
    pub fn obj(&self) -> Rc<O> {
        Rc::clone(&self.obj)
    }

    /// Returns reference to the state of this [`Component`].
    #[inline]
    #[must_use]
    pub fn state(&self) -> Rc<S> {
        Rc::clone(&self.state)
    }
}

impl<S: ComponentState<O> + 'static, O: 'static> Component<S, O> {
    /// Returns new [`Component`] with a provided object and state.
    ///
    /// Spawns all watchers of this [`Component`].
    pub fn new(obj: Rc<O>, state: Rc<S>) -> Self {
        let mut watchers_spawner =
            WatchersSpawner::new(Rc::clone(&state), Rc::clone(&obj));
        state.spawn_watchers(&mut watchers_spawner);

        Self {
            state,
            obj,
            _spawned_watchers: watchers_spawner.finish(),
        }
    }
}

/// Spawner for the [`Component`]'s watchers.
pub struct WatchersSpawner<S, O> {
    state: Rc<S>,
    obj: Rc<O>,
    spawned_watchers: Vec<TaskHandle>,
}

impl<S: 'static, O: 'static> WatchersSpawner<S, O> {
    /// Spawns watchers for the provided [`Stream`].
    ///
    /// If watcher returns an error then this error will be converted into the
    /// [`JasonError`] and printed with a [`JasonError::print()`].
    ///
    /// You can stop all listeners tasks spawned by this function by
    /// [`Drop`]ping [`Component`].
    pub fn spawn<R, V, F, H, E>(&mut self, mut rx: R, handle: F)
    where
        F: Fn(Rc<O>, Rc<S>, V) -> H + 'static,
        R: Stream<Item = V> + Unpin + 'static,
        H: Future<Output = Result<(), E>> + 'static,
        E: Into<JasonError>,
    {
        let obj = Rc::clone(&self.obj);
        let state = Rc::clone(&self.state);
        let (fut, handle) = future::abortable(async move {
            while let Some(value) = rx.next().await {
                if let Err(e) =
                    (handle)(Rc::clone(&obj), Rc::clone(&state), value).await
                {
                    Into::<JasonError>::into(e).print();
                }
            }
        });
        spawn_local(fut.map(|_| ()));

        self.spawned_watchers.push(handle.into());
    }

    /// Creates new [`WatchersSpawner`] for the provided object and state.
    #[inline]
    #[must_use]
    fn new(state: Rc<S>, obj: Rc<O>) -> Self {
        Self {
            state,
            obj,
            spawned_watchers: Vec::new(),
        }
    }

    /// Returns [`TaskHandle`]s for the watchers spawned by this
    /// [`WatchersSpawner`].
    #[inline]
    #[must_use]
    fn finish(self) -> Vec<TaskHandle> {
        self.spawned_watchers
    }
}

/// Abstraction describing state of the [`Component`].
pub trait ComponentState<C>: Sized {
    /// Spawns all watchers required for this [`ComponentState`].
    fn spawn_watchers(&self, spawner: &mut WatchersSpawner<Self, C>);
}

/// Helper trait for naming types of the [`Component`]'s state and object for
/// the [`ComponentState`] implementation generated by
/// [`medea_macro::watchers`].
pub trait ComponentTypes {
    /// Type of [`Component`]'s state.
    type State;

    /// Type of object managed by [`Component`].
    type Obj;
}

impl<S, O> ComponentTypes for Component<S, O> {
    type Obj = O;
    type State = S;
}
