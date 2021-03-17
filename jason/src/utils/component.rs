//! Implementation of the [`Component`].

use std::rc::Rc;

use derive_more::Deref;
use futures::{future, Future, FutureExt as _, Stream, StreamExt};
use medea_reactive::AllProcessed;
use wasm_bindgen_futures::spawn_local;

use crate::{
    media::LocalTracksConstraints,
    utils::{JasonError, TaskHandle},
};

/// Abstraction over a state which can be transformed to the states from the
/// [`medea_client_api_proto::state`].
pub trait AsProtoState {
    /// [`medea_client_api_proto::state`] into which this state can be
    /// transformed.
    type Output;

    /// Converts this state to the [`medea_client_api_proto::state`]
    /// representation.
    fn as_proto(&self) -> Self::Output;
}

/// Abstraction of state which can be updated or created by the
/// [`medea_client_api_proto::state`].
pub trait SynchronizableState {
    /// [`medea_client_api_proto::state`] by which this state can be updated.
    type Input;

    /// Creates a new state from the [`medea_client_api_proto::state`]
    /// representation.
    fn from_proto(
        input: Self::Input,
        send_cons: &LocalTracksConstraints,
    ) -> Self;

    /// Updates this state with a provided [`medea_client_api_proto::state`].
    fn apply(&self, input: Self::Input, send_cons: &LocalTracksConstraints);
}

/// Abstraction over a state which can be updated by a client side.
pub trait Updatable {
    /// Returns [`Future`] resolving once this [`Updatable`] state resolves its
    /// intentions.
    fn when_stabilized(&self) -> AllProcessed<'static>;

    /// Returns [`Future`] resolving once all the client updates are performed
    /// on this state.
    fn when_updated(&self) -> AllProcessed<'static>;

    /// Notifies about a RPC connection loss.
    fn connection_lost(&self);

    /// Notifies about a RPC connection recovering.
    fn connection_recovered(&self);
}

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
