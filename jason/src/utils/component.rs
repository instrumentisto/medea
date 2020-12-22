//! Implementation of the [`Component`].

use std::{cell::RefCell, ops::Deref, rc::Rc};

use futures::{
    future,
    future::{AbortHandle, LocalBoxFuture},
    Future, Stream, StreamExt,
};
use wasm_bindgen_futures::spawn_local;

use crate::utils::JasonError;

/// Abstraction over state which can be transformed to the states from the
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

    /// Creates state from the [`medea_client_api_proto::state`] representation.
    fn from_proto(input: Self::Input) -> Self;

    /// Updates this state with a provided [`medea_client_api_proto::state`].
    fn apply(&self, input: Self::Input);
}

/// Abstraction over state which can be updated by client side.
pub trait Updatable {
    /// Returns [`Future`] which will be resolved when all client updates will
    /// be performed on this state.
    fn when_updated(&self) -> LocalBoxFuture<'static, ()>;
}

/// Creates and spawns new [`Component`].
///
/// `$state` - type alias for [`Component`] which you wanna create.
///
/// `$state` - [`Component`]'s state wrapped to the [`Rc`].
///
/// `$ctx` - [`Component`]'s context wrapped to the [`Rc`].
///
/// `$global_ctx` - [`Component`]'s global context wrapped to the [`Rc`].
#[macro_export]
macro_rules! spawn_component {
    ($component:ty, $state:expr, $ctx:expr, $global_ctx:expr $(,)*) => {{
        let component = <$component>::new($state, $ctx, $global_ctx);
        component.spawn();
        component
    }};
}

/// Storage of the [`AbortHandle`]s used for aborting watchers tasks.
#[derive(Default)]
struct WatchersStorage(RefCell<Vec<AbortHandle>>);

impl WatchersStorage {
    /// Registers new [`AbortHandle`].
    #[inline]
    fn register_handle(&self, handle: AbortHandle) {
        self.0.borrow_mut().push(handle);
    }

    /// Aborts all spawned watchers tasks registered in this
    /// [`WatchersStorage`].
    #[inline]
    fn dispose(&self) {
        let handles: Vec<_> = std::mem::take(&mut self.0.borrow_mut());
        for handle in handles {
            handle.abort();
        }
    }
}

/// Base for all components of this app.
///
/// Can spawn new watchers with a [`Component::spawn_watcher`].
///
/// Will stop all spawned with a [`Component::spawn_watcher`] watchers on
/// [`Drop`].
///
/// Can be dereferenced to the [`Component`]'s context.
pub struct Component<S, C, G> {
    state: Rc<S>,
    ctx: Rc<C>,
    global_ctx: Rc<G>,
    watchers_store: WatchersStorage,
}

impl<S, C, G> Component<S, C, G> {
    /// Returns new [`Component`] with a provided data.
    #[inline]
    pub fn new(state: Rc<S>, ctx: Rc<C>, global_ctx: Rc<G>) -> Self {
        Self {
            state,
            ctx,
            global_ctx,
            watchers_store: WatchersStorage::default(),
        }
    }

    #[inline]
    pub fn global_ctx(&self) -> Rc<G> {
        self.global_ctx.clone()
    }

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

impl<S: 'static, C: 'static, G: 'static> Component<S, C, G> {
    /// Spawns watchers for the provided [`Stream`].
    ///
    /// If watcher returns error then this error will be converted to the
    /// [`JasonError`] and printed with a [`JasonError::print`].
    ///
    /// You can stop all listeners tasks spawned by this function by
    /// [`Component`] drop.
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
