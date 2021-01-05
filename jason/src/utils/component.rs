//! Implementation of the [`Component`].

use std::{cell::RefCell, ops::Deref, rc::Rc};

use futures::{future, Future, FutureExt, Stream, StreamExt};
use wasm_bindgen_futures::spawn_local;

use crate::utils::{JasonError, TaskHandle};

/// Creates and spawns new [`Component`].
///
/// `$component` - type alias for [`Component`] which you wanna create.
///
/// `$state` - [`Component`]'s state.
///
/// `$obj` - object managed by [`Component`].
#[macro_export]
macro_rules! spawn_component {
    ($component:ty, $state:expr, $obj:expr $(,)*) => {{
        let component = <$component>::inner_new($state, $obj);
        component.spawn();
        component
    }};
}

/// Component is a base that helps managing reactive components.
///
/// Component consists of two parts: state and object. Object is listening to
/// its state changes and updates accordingly, so all mutations are meant to be
/// applied to state.
pub struct Component<S, O> {
    state: Rc<S>,
    obj: Rc<O>,
    spawned_watchers: RefCell<Vec<TaskHandle>>,
}

impl<S, O> Component<S, O> {
    /// Returns new [`Component`] with a provided data. Not meant to be used
    /// directly, use `spawn_component` macro for creating components.
    #[inline]
    #[doc(hidden)]
    pub fn inner_new(state: Rc<S>, obj: Rc<O>) -> Self {
        Self {
            state,
            obj,
            spawned_watchers: RefCell::new(Vec::new()),
        }
    }

    /// Returns [`Rc`] to the object managed by this [`Component`].
    #[inline]
    pub fn obj(&self) -> Rc<O> {
        Rc::clone(&self.obj)
    }

    /// Returns reference to the state of this [`Component`]
    #[inline]
    pub fn state(&self) -> &S {
        &self.state
    }
}

impl<S: 'static, O: 'static> Component<S, O> {
    /// Spawns watchers for the provided [`Stream`].
    ///
    /// If watcher returns error then this error will be converted to the
    /// [`JasonError`] and printed with a [`JasonError::print`].
    ///
    /// You can stop all listeners tasks spawned by this function by
    /// [`Component`] drop.
    pub fn spawn_watcher<R, V, F, H, E>(&self, mut rx: R, handle: F)
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

        self.spawned_watchers.borrow_mut().push(handle.into());
    }
}

impl<S, O> Deref for Component<S, O> {
    type Target = O;

    fn deref(&self) -> &Self::Target {
        &self.obj
    }
}
