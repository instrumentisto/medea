use std::{
    cell::RefCell,
    fmt::{self, Debug, Error, Formatter},
    ops::{Deref, DerefMut},
};

use futures::{
    channel::{mpsc, oneshot},
    future::{self, LocalBoxFuture},
    stream::LocalBoxStream,
    StreamExt as _,
};

/// [`ReactiveField`] with which you can only subscribe on changes [`Stream`].
pub type DefaultReactiveField<T> =
    ReactiveField<T, Vec<UniversalSubscriber<T>>>;

/// A reactive cell which will emit all modification to the subscribers.
///
/// You can subscribe to this field modifications with
/// [`ReactiveField::subscribe`].
///
/// If you want to get [`Future`] which will be resolved only when data of this
/// field will become equal to some data, you can use [`ReactiveField::when`] or
/// [`ReactiveField::when_eq`].
pub struct ReactiveField<T, S> {
    /// Data which stored by this [`ReactiveField`].
    data: T,

    /// Subscribers on [`ReactiveField`]'s data mutations.
    subs: S,
}

impl<T, S> fmt::Debug for ReactiveField<T, S>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "ReactiveField {{ data: {:?} }}", self.data)
    }
}

impl<T> ReactiveField<T, Vec<UniversalSubscriber<T>>>
where
    T: 'static,
{
    /// Returns new [`ReactiveField`] on which mutations you can
    /// [`ReactiveSubscribe`], also you can subscribe on concrete mutation with
    /// [`ReactiveField::when`] and [`ReactiveField::when_eq`].
    pub fn new(data: T) -> Self {
        Self {
            data,
            subs: Vec::new(),
        }
    }
}

impl<T, S> ReactiveField<T, S>
where
    T: 'static,
    S: Subscribable<T>,
{
    /// Creates new [`ReactiveField`] with custom [`Subscribable`]
    /// implementation.
    pub fn new_with_custom(data: T, subs: S) -> Self {
        Self { data, subs }
    }
}

impl<T, S> ReactiveField<T, S>
where
    T: 'static,
    S: SubscribableOnce<T>,
{
    /// Returns [`Future`] which will be resolved only on modification with
    /// which your `assert_fn` returned `true`.
    pub fn when<F>(
        &mut self,
        assert_fn: F,
    ) -> LocalBoxFuture<'static, Result<(), Dropped>>
    where
        F: Fn(&T) -> bool + 'static,
    {
        if (assert_fn)(&self.data) {
            Box::pin(future::ok(()))
        } else {
            self.subs.when(Box::new(assert_fn))
        }
    }
}

impl<T, S> ReactiveField<T, S>
where
    S: Subscribable<T>,
    T: Clone + 'static,
{
    pub fn subscribe(&mut self) -> LocalBoxStream<'static, T> {
        let data = self.data.clone();
        let subscription = self.subs.subscribe();

        Box::pin(futures::stream::once(async move { data }).chain(subscription))
    }
}

impl<T, S> ReactiveField<T, S>
where
    T: Eq + 'static,
    S: SubscribableOnce<T>,
{
    /// Returns [`Future`] which will be resolved only when data of this
    /// [`ReactiveField`] will become equal to provided `should_be`.
    pub fn when_eq(
        &mut self,
        should_be: T,
    ) -> LocalBoxFuture<'static, Result<(), Dropped>> {
        self.when(move |data| data == &should_be)
    }
}

impl<T, S> ReactiveField<T, S>
where
    S: OnReactiveFieldModification<T>,
    T: Clone + Eq,
{
    /// Returns [`MutReactiveFieldGuard`] which can be mutably dereferenced to
    /// underlying data.
    ///
    /// If some mutation of data happened between calling
    /// [`ReactiveField::borrow_mut`] and dropping of
    /// [`MutReactiveFieldGuard`], then all subscribers of this
    /// [`ReactiveField`] will be notified about this.
    ///
    /// Notification about mutation will be sent only if this field __really__
    /// changed. This will be checked with [`PartialEq`] implementation of
    /// underlying data.
    pub fn borrow_mut(&mut self) -> MutReactiveFieldGuard<'_, T, S> {
        MutReactiveFieldGuard {
            value_before_mutation: self.data.clone(),
            data: &mut self.data,
            subs: &mut self.subs,
        }
    }
}

pub trait OnReactiveFieldModification<T> {
    /// This function will be called on every [`ReactiveField`] modification.
    ///
    /// On this function call subsciber which implements
    /// [`OnReactiveFieldModification`] should send a update to a [`Stream`]
    /// or resolve [`Future`].
    fn on_modify(&mut self, data: &T);
}

pub trait Subscribable<T: 'static> {
    /// This function will be called on [`ReactiveField::subscribe`].
    ///
    /// Should return [`LocalBoxStream`] to which will be sent data updates.
    fn subscribe(&mut self) -> LocalBoxStream<'static, T>;
}

/// Subscriber which implements [`Subscribable`] and [`SubscribableOnce`] in
/// [`Vec`].
///
/// This structure should be wrapped into [`Vec`].
pub enum UniversalSubscriber<T> {
    When {
        sender: RefCell<Option<oneshot::Sender<()>>>,
        assert_fn: Box<dyn Fn(&T) -> bool>,
    },
    All(mpsc::UnboundedSender<T>),
}

/// Error will be sent to all subscribers when this [`ReactiveField`] is
/// dropped.
#[derive(Debug)]
pub struct Dropped;

impl From<oneshot::Canceled> for Dropped {
    fn from(_: oneshot::Canceled) -> Self {
        Self
    }
}

pub trait SubscribableOnce<T: 'static> {
    /// This function will be called on [`ReactiveField::when`].
    ///
    /// Should return [`LocalBoxFuture`] to which will be sent `()` when
    /// provided `assert_fn` returns `true`.
    fn when(
        &mut self,
        assert_fn: Box<dyn Fn(&T) -> bool>,
    ) -> LocalBoxFuture<'static, Result<(), Dropped>>;
}

impl<T: 'static> SubscribableOnce<T> for Vec<UniversalSubscriber<T>> {
    fn when(
        &mut self,
        assert_fn: Box<dyn Fn(&T) -> bool>,
    ) -> LocalBoxFuture<'static, Result<(), Dropped>> {
        let (tx, rx) = oneshot::channel();
        self.push(UniversalSubscriber::When {
            sender: RefCell::new(Some(tx)),
            assert_fn,
        });

        Box::pin(async move { Ok(rx.await?) })
    }
}

impl<T: 'static> Subscribable<T> for Vec<UniversalSubscriber<T>> {
    fn subscribe(&mut self) -> LocalBoxStream<'static, T> {
        let (tx, rx) = mpsc::unbounded();
        self.push(UniversalSubscriber::All(tx));

        Box::pin(rx)
    }
}

impl<T: Clone> OnReactiveFieldModification<T> for Vec<UniversalSubscriber<T>> {
    fn on_modify(&mut self, data: &T) {
        self.retain(|sub| match sub {
            UniversalSubscriber::When { assert_fn, sender } => {
                if (assert_fn)(data) {
                    sender.borrow_mut().take().unwrap().send(()).ok();
                    true
                } else {
                    false
                }
            }
            UniversalSubscriber::All(sender) => {
                sender.unbounded_send(data.clone()).unwrap();
                false
            }
        });
    }
}

impl<T, S> Deref for ReactiveField<T, S> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

pub struct MutReactiveFieldGuard<'a, T, S>
where
    S: OnReactiveFieldModification<T>,
    T: Eq,
{
    data: &'a mut T,
    subs: &'a mut S,
    value_before_mutation: T,
}

impl<'a, T, S> Deref for MutReactiveFieldGuard<'a, T, S>
where
    S: OnReactiveFieldModification<T>,
    T: Eq,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'a, T, S> DerefMut for MutReactiveFieldGuard<'a, T, S>
where
    S: OnReactiveFieldModification<T>,
    T: Eq,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<'a, T, S> Drop for MutReactiveFieldGuard<'a, T, S>
where
    S: OnReactiveFieldModification<T>,
    T: Eq,
{
    fn drop(&mut self) {
        if self.data != &self.value_before_mutation {
            self.subs.on_modify(&self.data);
        }
    }
}
