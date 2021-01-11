//! Implementation of the [`Component`].

use std::{cell::RefCell, ops::Deref, rc::Rc};

use futures::{future, future::AbortHandle, Future, Stream, StreamExt};
use medea_reactive::RecheckableFutureExt;
use wasm_bindgen_futures::spawn_local;

use crate::utils::JasonError;
use futures::future::LocalBoxFuture;

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
    fn when_stabilized(&self) -> LocalBoxFuture<'static, ()>;


    /// Returns [`Future`] which will be resolved when all client updates will
    /// be performed on this state.
    fn when_updated(&self) -> Box<dyn RecheckableFutureExt<Output = ()>>;
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
    ($component:ty, $state:expr, $ctx:expr $(,)*) => {{
        let component = <$component>::inner_new($state, $ctx);
        component.spawn();
        component
    }};
}

/// Base for all components of this app.
///
/// Can spawn new watchers with a [`Component::spawn_watcher`].
///
/// Will stop all spawned with a [`Component::spawn_watcher`] watchers on
/// [`Drop`].
///
/// Can be dereferenced to the [`Component`]'s context.
pub struct Component<S, C> {
    state: Rc<S>,
    ctx: Rc<C>,
    watchers_store: RefCell<Vec<AbortHandle>>,
}

impl<S, C> Component<S, C> {
    /// Returns new [`Component`] with a provided data.
    #[inline]
    pub fn inner_new(state: Rc<S>, ctx: Rc<C>) -> Self {
        Self {
            state,
            ctx,
            watchers_store: RefCell::new(Vec::new()),
        }
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

    // TODO (evdokimovs): Remove this function.
    pub fn rc_state(&self) -> Rc<S> {
        Rc::clone(&self.state)
    }
}

impl<S: 'static, C: 'static> Component<S, C> {
    /// Spawns watchers for the provided [`Stream`].
    ///
    /// If watcher returns error then this error will be converted to the
    /// [`JasonError`] and printed with a [`JasonError::print`].
    ///
    /// You can stop all listeners tasks spawned by this function by
    /// [`Component`] drop.
    pub fn spawn_watcher<R, V, F, O, E>(&self, mut rx: R, handle: F)
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
        self.watchers_store.borrow_mut().push(handle);
    }
}

impl<S, C> Drop for Component<S, C> {
    fn drop(&mut self) {
        let handles = self.watchers_store.replace(Vec::default());
        for handle in handles {
            handle.abort();
        }
    }
}

impl<S, C> Deref for Component<S, C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.ctx
    }
}
