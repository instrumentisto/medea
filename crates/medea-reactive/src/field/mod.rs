//! Implementations of basic reactive containers.

#![allow(clippy::module_name_repetitions)]

pub mod cell;
pub mod progressable_cell;

use std::{
    cell::RefCell,
    fmt,
    ops::{Deref, DerefMut},
};

use futures::{
    channel::{mpsc, oneshot},
    future::LocalBoxFuture,
    stream::{self, LocalBoxStream, StreamExt as _},
};

use crate::subscribers_store::{
    progressable, progressable::Processed, SubscribersStore,
};

#[doc(inline)]
pub use self::{cell::ObservableCell, progressable_cell::ProgressableCell};

/// Default type of [`ObservableField`] subscribers.
type DefaultSubscribers<D> = RefCell<Vec<UniversalSubscriber<D>>>;

/// [`ObservableField`] that allows to subscribe to all changes
/// ([`ObservableField::subscribe`]) and to concrete changes
/// ([`ObservableField::when`] and [`ObservableField::when_eq`]).
pub type Observable<D> = ObservableField<D, DefaultSubscribers<D>>;

/// [`ObservableField`] that allows to subscribe to all changes
/// ([`ObservableField::subscribe`]) and to concrete changes
/// ([`ObservableField::when`] and [`ObservableField::when_eq`]).
///
/// Can recognise when all updates was processed by subscribers.
pub type Progressable<D> = ObservableField<D, progressable::SubStore<D>>;

/// Reactive cell which emits all modifications to its subscribers.
///
/// Subscribing to this field modifications is done with
/// [`ObservableField::subscribe`] method.
///
/// If you want to get [`Future`] which will resolved only when an underlying
/// data of this field will become equal to some value, you can use
/// [`ObservableField::when`] or [`ObservableField::when_eq`] methods.
///
/// [`Future`]: std::future::Future
#[derive(Debug)]
pub struct ObservableField<D, S> {
    /// Data which is stored by this [`ObservableField`].
    data: D,

    /// Subscribers to [`ObservableField`]'s data mutations.
    subs: S,
}

impl<D> ObservableField<D, RefCell<Vec<UniversalSubscriber<D>>>>
where
    D: 'static,
{
    /// Returns new [`ObservableField`] with subscribable mutations.
    ///
    /// Also you can subscribe to concrete mutations with
    /// [`ObservableField::when`] and [`ObservableField::when_eq`] methods.
    #[inline]
    pub fn new(data: D) -> Self {
        Self {
            data,
            subs: RefCell::new(Vec::new()),
        }
    }
}

impl<D, S> ObservableField<D, S>
where
    D: 'static,
    S: Whenable<D>,
{
    /// Returns [`Future`] which will resolve only on modifications that
    /// the given `assert_fn` returns `true` on.
    ///
    /// [`Future`]: std::future::Future
    pub fn when<F>(
        &self,
        assert_fn: F,
    ) -> LocalBoxFuture<'static, Result<(), DroppedError>>
    where
        F: Fn(&D) -> bool + 'static,
    {
        // TODO: This is kinda broken.
        //       See https://github.com/instrumentisto/medea/issues/163 issue.
        if (assert_fn)(&self.data) {
            Box::pin(futures::future::ok(()))
        } else {
            self.subs.when(Box::new(assert_fn))
        }
    }
}

impl<D: 'static> Progressable<D> {
    /// Returns new [`ObservableField`] with subscribable mutations.
    ///
    /// Also, you can wait for all updates processing by awaiting on
    /// [`ObservableField::when_all_processed()`].
    #[inline]
    pub fn new(data: D) -> Self {
        Self {
            data,
            subs: progressable::SubStore::default(),
        }
    }
}

impl<D> Progressable<D>
where
    D: Clone + 'static,
{
    /// Returns [`Stream`] into which underlying data updates (wrapped in the
    /// [`progressable::Guarded`]) will be emitted.
    ///
    /// [`Stream`]: futures::Stream
    pub fn subscribe(
        &self,
    ) -> LocalBoxStream<'static, progressable::Guarded<D>> {
        let data = self.subs.wrap(self.data.clone());
        Box::pin(stream::once(async move { data }).chain(self.subs.subscribe()))
    }

    /// Returns [`Future`] resolving when all data updates will be processed by
    /// subscribers.
    ///
    /// [`Future`]: std::future::Future
    pub fn when_all_processed(&self) -> Processed<'static> {
        self.subs.when_all_processed()
    }
}

impl<D> Observable<D>
where
    D: Clone + 'static,
{
    /// Returns [`Stream`] into which underlying data updates will be emitted.
    ///
    /// [`Stream`]: futures::Stream
    pub fn subscribe(&self) -> LocalBoxStream<'static, D> {
        let data = self.data.clone();
        let (tx, rx) = mpsc::unbounded();
        self.subs
            .borrow_mut()
            .push(UniversalSubscriber::Subscribe(tx));

        Box::pin(stream::once(async move { data }).chain(Box::pin(rx)))
    }
}

impl<D, S> ObservableField<D, S>
where
    D: PartialEq + 'static,
    S: Whenable<D>,
{
    /// Returns [`Future`] which will resolve only when an underlying data of
    /// this [`ObservableField`] will become equal to the provided `should_be`
    /// value.
    ///
    /// [`Future`]: std::future::Future
    // TODO: This is kinda broken.
    //       See https://github.com/instrumentisto/medea/issues/163 issue.
    #[inline]
    pub fn when_eq(
        &self,
        should_be: D,
    ) -> LocalBoxFuture<'static, Result<(), DroppedError>> {
        self.when(move |data| data == &should_be)
    }
}

impl<D, S> ObservableField<D, S>
where
    S: OnObservableFieldModification<D>,
    D: Clone + PartialEq,
{
    /// Returns [`MutObservableFieldGuard`] which can be mutably dereferenced to
    /// an underlying data.
    ///
    /// If some mutation of data happens between calling
    /// [`ObservableField::borrow_mut`] and dropping of
    /// [`MutObservableFieldGuard`], then all subscribers of this
    /// [`ObservableField`] will be notified about this.
    ///
    /// Notification about mutation will be sent only if this field __really__
    /// changed. This will be checked with [`PartialEq`] implementation of
    /// underlying data.
    #[inline]
    pub fn borrow_mut(&mut self) -> MutObservableFieldGuard<'_, D, S> {
        MutObservableFieldGuard {
            value_before_mutation: self.data.clone(),
            data: &mut self.data,
            subs: &mut self.subs,
        }
    }
}

/// Abstraction over catching all unique modifications of an
/// [`ObservableField`].
pub trait OnObservableFieldModification<D> {
    /// This function will be called on each [`ObservableField`]'s modification.
    ///
    /// On this function call subscriber (which implements
    /// [`OnObservableFieldModification`]) should send an update to [`Stream`]
    /// or resolve [`Future`].
    ///
    /// [`Future`]: std::future::Future
    /// [`Stream`]: futures::Stream
    fn on_modify(&mut self, data: &D);
}

/// Subscriber that implements subscribing and [`Whenable`] in [`Vec`].
///
/// This structure should be wrapped into [`Vec`].
pub enum UniversalSubscriber<D> {
    /// Subscriber for [`Whenable`].
    When {
        /// [`oneshot::Sender`] with which [`Whenable::when`]'s [`Future`] will
        /// resolve.
        ///
        /// [`Future`]: std::future::Future
        sender: RefCell<Option<oneshot::Sender<()>>>,

        /// Function with which will be checked that [`Whenable::when`]'s
        /// [`Future`] should resolve.
        ///
        /// [`Future`]: std::future::Future
        assert_fn: Box<dyn Fn(&D) -> bool>,
    },

    /// Subscriber for data updates.
    Subscribe(mpsc::UnboundedSender<D>),
}

impl<D> fmt::Debug for UniversalSubscriber<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            UniversalSubscriber::When { .. } => {
                write!(f, "UniversalSubscriber::When")
            }
            UniversalSubscriber::Subscribe(_) => {
                write!(f, "UniversalSubscriber::Subscribe")
            }
        }
    }
}

/// Error that is sent to all subscribers when this [`ObservableField`] /
/// [`ObservableCell`] is dropped.
#[derive(Clone, Copy, Debug)]
pub struct DroppedError;

impl fmt::Display for DroppedError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Observable value has been dropped")
    }
}

impl From<oneshot::Canceled> for DroppedError {
    #[inline]
    fn from(_: oneshot::Canceled) -> Self {
        Self
    }
}

/// Abstraction over [`ObservableField::when`] and [`ObservableField::when_eq`]
/// implementations for custom types.
pub trait Whenable<D: 'static> {
    /// This function will be called on [`ObservableField::when`].
    ///
    /// Should return [`LocalBoxFuture`] to which will be sent `()` when
    /// provided `assert_fn` returns `true`.
    fn when(
        &self,
        assert_fn: Box<dyn Fn(&D) -> bool>,
    ) -> LocalBoxFuture<'static, Result<(), DroppedError>>;
}

#[allow(clippy::use_self)]
impl<D: 'static> Whenable<D> for RefCell<Vec<UniversalSubscriber<D>>> {
    fn when(
        &self,
        assert_fn: Box<dyn Fn(&D) -> bool>,
    ) -> LocalBoxFuture<'static, Result<(), DroppedError>> {
        let (tx, rx) = oneshot::channel();
        self.borrow_mut().push(UniversalSubscriber::When {
            sender: RefCell::new(Some(tx)),
            assert_fn,
        });
        Box::pin(async move { Ok(rx.await?) })
    }
}

impl<D: Clone + 'static> OnObservableFieldModification<D>
    for progressable::SubStore<D>
{
    fn on_modify(&mut self, data: &D) {
        self.send_update(data.clone());
    }
}

impl<D: Clone> OnObservableFieldModification<D>
    for RefCell<Vec<UniversalSubscriber<D>>>
{
    fn on_modify(&mut self, data: &D) {
        self.borrow_mut().retain(|sub| match sub {
            UniversalSubscriber::When { assert_fn, sender } => {
                if (assert_fn)(data) {
                    let _ = sender.borrow_mut().take().unwrap().send(());
                    false
                } else {
                    true
                }
            }
            UniversalSubscriber::Subscribe(sender) => {
                sender.unbounded_send(data.clone()).is_ok()
            }
        });
    }
}

impl<D, S> Deref for ObservableField<D, S> {
    type Target = D;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<D, S> fmt::Display for ObservableField<D, S>
where
    D: fmt::Display,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.data, f)
    }
}

/// Mutable [`ObservableField`] reference returned by
/// [`ObservableField::borrow_mut`].
///
/// When this guard is [`Drop`]ped, a check for modifications will be performed.
/// If data was changed, then [`OnObservableFieldModification::on_modify`] will
/// be called.
#[derive(Debug)]
pub struct MutObservableFieldGuard<'a, D, S>
where
    S: OnObservableFieldModification<D>,
    D: PartialEq,
{
    /// Data stored by this [`ObservableField`].
    data: &'a mut D,

    /// Subscribers to [`ObservableField`]'s data mutations.
    subs: &'a mut S,

    /// Data stored by this [`ObservableField`] before mutation.
    value_before_mutation: D,
}

impl<'a, D, S> Deref for MutObservableFieldGuard<'a, D, S>
where
    S: OnObservableFieldModification<D>,
    D: PartialEq,
{
    type Target = D;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'a, D, S> DerefMut for MutObservableFieldGuard<'a, D, S>
where
    S: OnObservableFieldModification<D>,
    D: PartialEq,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<'a, D, S> Drop for MutObservableFieldGuard<'a, D, S>
where
    S: OnObservableFieldModification<D>,
    D: PartialEq,
{
    #[inline]
    fn drop(&mut self) {
        if self.data != &self.value_before_mutation {
            self.subs.on_modify(&self.data);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, time::Duration};

    use futures::{poll, task::Poll, StreamExt as _};
    use tokio::time::timeout;

    use crate::{Observable, Progressable};

    #[tokio::test]
    async fn subscriber_receives_current_data() {
        let field = Observable::new(9);
        let current_data = field.subscribe().next().await.unwrap();
        assert_eq!(current_data, 9);
    }

    #[tokio::test]
    async fn when_eq_resolves_if_value_eq_already() {
        let field = Observable::new(9);
        field.when_eq(9).await.unwrap();
    }

    #[tokio::test]
    async fn when_eq_doesnt_resolve_if_value_is_not_eq() {
        let field = Observable::new(9);
        let _ = timeout(Duration::from_millis(50), field.when_eq(0))
            .await
            .unwrap_err();
    }

    #[tokio::test]
    async fn current_value_is_provided_into_assert_fn_on_when_call() {
        let field = Observable::new(9);

        timeout(Duration::from_millis(50), field.when(|val| val == &9))
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn value_updates_are_sent_to_subs() {
        let mut field = Observable::new(0);
        let mut subscription_on_changes = field.subscribe();

        for _ in 0..100 {
            *field.borrow_mut() += 1;
        }
        loop {
            if let Some(change) = subscription_on_changes.next().await {
                if change == 100 {
                    break;
                }
            } else {
                panic!("Stream ended too early!");
            }
        }
    }

    #[tokio::test]
    async fn when_resolves_on_value_update() {
        let mut field = Observable::new(0);
        let subscription = field.when(|change| change == &100);

        for _ in 0..100 {
            *field.borrow_mut() += 1;
        }

        timeout(Duration::from_millis(50), subscription)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn when_eq_resolves_on_value_update() {
        let mut field = Observable::new(0);
        let subscription = field.when_eq(100);

        for _ in 0..100 {
            *field.borrow_mut() += 1;
        }

        timeout(Duration::from_millis(50), subscription)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn when_returns_dropped_error_on_drop() {
        let field = Observable::new(0);
        let subscription = field.when(|change| change == &100);
        drop(field);
        let _ = subscription.await.unwrap_err();
    }

    #[tokio::test]
    async fn when_eq_returns_dropped_error_on_drop() {
        let field = Observable::new(0);
        let subscription = field.when_eq(100);
        drop(field);
        let _ = subscription.await.unwrap_err();
    }

    #[tokio::test]
    async fn stream_ends_when_reactive_field_is_dropped() {
        let field = Observable::new(0);
        let subscription = field.subscribe();
        drop(field);
        assert!(subscription.skip(1).next().await.is_none());
    }

    #[tokio::test]
    async fn no_update_should_be_emitted_on_field_mutation() {
        let mut field = Observable::new(0);
        let subscription = field.subscribe();
        *field.borrow_mut() = 0;
        let _ = timeout(
            Duration::from_millis(50),
            Box::pin(subscription.skip(1).next()),
        )
        .await
        .unwrap_err();
    }

    #[tokio::test]
    async fn only_last_update_should_be_sent_to_subscribers() {
        let mut field = Observable::new(0);
        let subscription = field.subscribe();
        let mut field_mut_guard = field.borrow_mut();
        *field_mut_guard = 100;
        *field_mut_guard = 200;
        *field_mut_guard = 300;
        drop(field_mut_guard);
        assert_eq!(subscription.skip(1).next().await.unwrap(), 300);
    }

    #[tokio::test]
    async fn reactive_with_refcell_inside() {
        let field = RefCell::new(Observable::new(0));
        let subscription = field.borrow().when_eq(1);
        *field.borrow_mut().borrow_mut() = 1;
        timeout(Duration::from_millis(50), Box::pin(subscription))
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn when_all_processed_works() {
        let mut field = Progressable::new(1);
        assert_eq!(poll!(field.when_all_processed()), Poll::Ready(()));
        *field.borrow_mut() = 2;
        assert_eq!(poll!(field.when_all_processed()), Poll::Ready(()));

        let mut subscribe = field.subscribe();
        assert_eq!(poll!(field.when_all_processed()), Poll::Pending);

        assert_eq!(subscribe.next().await.unwrap().into_inner(), 2);
        *field.borrow_mut() = 3;
        assert_eq!(poll!(field.when_all_processed()), Poll::Pending);
        assert_eq!(subscribe.next().await.unwrap().into_inner(), 3);
        assert_eq!(poll!(field.when_all_processed()), Poll::Ready(()));
    }
}
