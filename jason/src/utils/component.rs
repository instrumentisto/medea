//! Implementation of the [`Component`].

use std::{cell::RefCell, ops::Deref, rc::Rc};

use futures::{future, future::AbortHandle, Future, Stream, StreamExt};
use wasm_bindgen_futures::spawn_local;

use crate::utils::JasonError;

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
    watchers_store: RefCell<Vec<AbortHandle>>,
}

impl<S, C, G> Component<S, C, G> {
    /// Returns new [`Component`] with a provided data.
    #[inline]
    pub fn new(state: Rc<S>, ctx: Rc<C>, global_ctx: Rc<G>) -> Self {
        Self {
            state,
            ctx,
            global_ctx,
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
        let ctx = Rc::clone(&self.ctx);
        let global_ctx = Rc::clone(&self.global_ctx);
        let state = Rc::clone(&self.state);
        let (fut, handle) = future::abortable(async move {
            while let Some(value) = rx.next().await {
                if let Err(e) = (handle)(
                    Rc::clone(&ctx),
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
        self.watchers_store.borrow_mut().push(handle);
    }
}

impl<S, C, G> Drop for Component<S, C, G> {
    fn drop(&mut self) {
        let handles = self.watchers_store.replace(Vec::default());
        for handle in handles {
            handle.abort();
        }
    }
}

impl<S, C, G> Deref for Component<S, C, G> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.ctx
    }
}
