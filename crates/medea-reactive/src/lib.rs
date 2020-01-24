//! Containers to which data modifictions you can subscribe.
//!
//! # Basic iteraction with a `ObservableField`
//!
//! ## With primitive
//!
//! ```
//! use medea_reactive::Observable;
//!
//! // Create a reactive field with 'u32' data inside.
//! let mut foo = Observable::new(0u32);
//!
//! // If you want to get data which held by 'ObservableField' you can
//! // just deref 'ObservableField' container:
//! assert_eq!(*foo + 100, 100);
//!
//! // If you want to modify data which 'ObservableField' holds, you should use
//! // '.borrow_mut()':
//! *foo.borrow_mut() = 0;
//! assert_eq!(*foo, 0);
//! ```
//!
//! ## With object
//!
//! ```
//! use medea_reactive::Observable;
//!
//! // 'ObservableField' only works with objects which implements
//! // 'Clone' and 'PartialEq'.
//! #[derive(Clone, PartialEq)]
//! struct Foo(u32);
//!
//! impl Foo {
//!     pub fn new() -> Self {
//!         Self(0)
//!     }
//!
//!     pub fn increase(&mut self) {
//!         self.0 += 1;
//!     }
//!
//!     pub fn current_num(&self) -> u32 {
//!         self.0
//!     }
//! }
//!
//! let mut foo = Observable::new(Foo::new());
//! // You can transparently call methods of object which stored by
//! // 'ObservableField' if they not mutate.
//! assert_eq!(foo.current_num(), 0);
//! // If you want to call mutable method of object which stored by
//! // 'ObservableField' you should call '.borrow_mut()' before it.
//! foo.borrow_mut().increase();
//! assert_eq!(foo.current_num(), 1);
//! ```
//!
//! # Subscription on all `ObservableField` data modification
//!
//! ```
//! use medea_reactive::Observable;
//! # use futures::{executor, StreamExt as _};
//!
//! # executor::block_on(async {
//! let mut foo = Observable::new(0u32);
//! // Subscribe on all field modifications:
//! let mut foo_changes_stream = foo.subscribe();
//!
//! // Initial 'ObservableField' field data will be sent as modification:
//! assert_eq!(foo_changes_stream.next().await.unwrap(), 0);
//!
//! // Modify 'ObservableField' field:
//! *foo.borrow_mut() = 1;
//! // Receive modification update:
//! assert_eq!(foo_changes_stream.next().await.unwrap(), 1);
//!
//! // On this mutable borrow, field actually not changed:
//! *foo.borrow_mut() = 1;
//! // After this we really modify value:
//! *foo.borrow_mut() = 2;
//! // Only real modification was sent to the modification subscriber:
//! assert_eq!(foo_changes_stream.next().await.unwrap(), 2);
//! # });
//! ```
//!
//! # Subscription on concrete `ObservableField` data modification
//!
//! ```
//! use medea_reactive::Observable;
//! # use futures::{executor, StreamExt as _};
//!
//! # executor::block_on(async {
//! let mut foo = Observable::new(0u32);
//!
//! // Create future which will be resolved when foo will become one:
//! let when_foo_will_be_one = foo.when_eq(1);
//! *foo.borrow_mut() = 1;
//! // Future resolves because value was become 1.
//! when_foo_will_be_one.await.unwrap();
//!
//! // Or you can define your own resolve logic:
//! let when_foo_will_be_greated_than_5 = foo.when(|foo_upd| foo_upd > &5);
//! *foo.borrow_mut() = 6;
//! // Future resolves because value was become greater than 5.
//! when_foo_will_be_greated_than_5.await.unwrap();
//! # });
//! ```
//!
//! # Hold mutable reference of `ObservableField`
//!
//! ```
//! use medea_reactive::Observable;
//! # use futures::{executor, StreamExt as _};
//!
//! # executor::block_on(async {
//! let mut foo = Observable::new(0u32);
//!
//! // Subscribe on all 'foo' changes:
//! let mut foo_changes_stream = foo.subscribe();
//! // Just skip initial 'ObservableField' value:
//! assert_eq!(foo_changes_stream.next().await.unwrap(), 0);
//! // And hold mutable reference in variable:
//! let mut foo_mut_ref = foo.borrow_mut();
//! // First change:
//! *foo_mut_ref = 100;
//! // Second change:
//! *foo_mut_ref = 200;
//! // Drop mutable reference because only on mutable reference drop changes
//! // will be checked:
//! drop(foo_mut_ref);
//! // Only last change was sent into change stream:
//! assert_eq!(foo_changes_stream.next().await.unwrap(), 200);
//!
//! let mut foo_changes_subscription = foo.subscribe();
//! let mut foo_mut_ref = foo.borrow_mut();
//! // Really change data of 'ObservableField' here:
//! *foo_mut_ref = 100;
//! // But at the end we're reverted to a original value:
//! *foo_mut_ref = 200;
//! // Drop mutable reference because only on mutable reference drop changes
//! // will be checked:
//! drop(foo_mut_ref);
//! // No changes will be sent into 'foo_change_subscription' stream,
//! // because only last value checked.
//! # })
//! ```

#![allow(clippy::module_name_repetitions, clippy::must_use_candidate)]
#![warn(missing_docs)]

use std::{
    cell::{Ref, RefCell},
    fmt::{self, Debug, Error, Formatter},
    ops::{Deref, DerefMut},
};

use futures::{
    channel::{mpsc, oneshot},
    future::{self, LocalBoxFuture},
    stream::{self, LocalBoxStream},
    StreamExt as _,
};

type DefaultSubscribers<D> = RefCell<Vec<UniversalSubscriber<D>>>;

/// [`ObservableField`] with which you can only subscribe on changes
/// ([`ObservableField::subscribe`]) and on concrete changes
/// ([`ObservableField::when`] and [`ObservableField::when_eq`]).
pub type Observable<D> = ObservableField<D, DefaultSubscribers<D>>;

/// Observable analog for the [`Cell`].
///
/// Subscription on changes works same as [`ObservableField`],
/// but working with underlying data of [`ObservableCell`] is differs.
///
/// # `ObservableCell` underlying data accessing
///
/// ## For copyable types
///
/// ```
/// use medea_reactive::ObservableCell;
///
/// let foo = ObservableCell::new(0i32);
///
/// // If data implements 'Copy' then you can get copy of current value:
/// assert_eq!(foo.get(), 0);
/// ```
///
/// ## Reference to a underlying data
///
/// ```
/// use medea_reactive::ObservableCell;
///
/// struct Foo(i32);
///
/// impl Foo {
///     pub fn new(num: i32) -> Self {
///         Self(num)
///     }
///
///     pub fn get_num(&self) -> i32 {
///         self.0
///     }
/// }
///
/// let foo = ObservableCell::new(Foo::new(100));
/// assert_eq!(foo.borrow().get_num(), 100);
/// ```
///
/// # `ObservableCell` data mutating
///
/// ```
/// use medea_reactive::ObservableCell;
///
/// let foo = ObservableCell::new(0i32);
///
/// // You can just set some data:
/// foo.set(100);
/// assert_eq!(foo.get(), 100);
///
/// // Or replace data with new data and get old data:
/// let old_value = foo.replace(200);
/// assert_eq!(old_value, 100);
/// assert_eq!(foo.get(), 200);
///
/// // Or mutate this data:
/// foo.mutate(|mut data| *data = 300);
/// assert_eq!(foo.get(), 300);
/// ```
///
/// [`Cell`]: std::cell::Cell
pub struct ObservableCell<D>(RefCell<Observable<D>>);

impl<D> ObservableCell<D>
where
    D: 'static,
{
    /// Returns new [`ObservableCell`] on which mutations you can
    /// subscribe, also you can subscribe on concrete mutation
    /// with [`ObservableCell::when`] and [`ObservableCell::when_eq`].
    ///
    /// This container can mutate internally. Read docs of [`ObservableCell`]
    /// for more info.
    pub fn new(data: D) -> Self {
        Self(RefCell::new(Observable::new(data)))
    }

    /// Returns immutable reference to a underlying data.
    pub fn borrow(&self) -> Ref<D> {
        let reference = self.0.borrow();
        Ref::map(reference, |observable| observable.deref())
    }

    /// Returns [`Future`] which will be resolved only on modification with
    /// which your `assert_fn` returned `true`.
    ///
    /// [`Future`]: std::future::Future
    pub fn when<F>(
        &self,
        assert_fn: F,
    ) -> LocalBoxFuture<'static, Result<(), Dropped>>
    where
        F: Fn(&D) -> bool + 'static,
    {
        self.0.borrow().when(assert_fn)
    }
}

impl<D> ObservableCell<D>
where
    D: Copy + 'static,
{
    /// Returns copy of underlying data.
    pub fn get(&self) -> D {
        **self.0.borrow()
    }
}

impl<D> ObservableCell<D>
where
    D: Clone + 'static,
{
    /// Returns [`Stream`] into which underlying data updates will be emitted.
    ///
    /// [`Stream`]: futures::Stream
    pub fn subscribe(&self) -> LocalBoxStream<'static, D> {
        self.0.borrow().subscribe()
    }
}

impl<D> ObservableCell<D>
where
    D: PartialEq + 'static,
{
    /// Returns [`Future`] which will be resolved only when data of this
    /// [`ObservableCell`] will become equal to provided `should_be`.
    ///
    /// [`Future`]: std::future::Future
    pub fn when_eq(
        &self,
        should_be: D,
    ) -> LocalBoxFuture<'static, Result<(), Dropped>> {
        self.0.borrow().when_eq(should_be)
    }
}

impl<D> ObservableCell<D>
where
    D: Clone + PartialEq + 'static,
{
    /// Sets the contained value.
    pub fn set(&self, new_data: D) {
        *self.0.borrow_mut().borrow_mut() = new_data;
    }

    /// Replaces the contained value, and returns it.
    pub fn replace(&self, mut new_data: D) -> D {
        std::mem::swap(
            self.0.borrow_mut().borrow_mut().deref_mut(),
            &mut new_data,
        );
        new_data
    }

    /// Updates the contained value using a function to which will be provided
    /// mutable reference to a underlying data.
    pub fn mutate<F>(&self, f: F)
    where
        F: FnOnce(MutObservableFieldGuard<D, DefaultSubscribers<D>>),
    {
        (f)(self.0.borrow_mut().borrow_mut());
    }
}

/// A reactive cell which will emit all modification to the subscribers.
///
/// You can subscribe to this field modifications with
/// [`ObservableField::subscribe`].
///
/// If you want to get [`Future`] which will be resolved only when data of this
/// field will become equal to some data, you can use [`ObservableField::when`]
/// or [`ObservableField::when_eq`].
///
/// [`Future`]: std::future::Future
pub struct ObservableField<D, S> {
    /// Data which stored by this [`ObservableField`].
    data: D,

    /// Subscribers on [`ObservableField`]'s data mutations.
    subs: S,
}

impl<D> ObservableField<D, RefCell<Vec<UniversalSubscriber<D>>>>
where
    D: 'static,
{
    /// Returns new [`ObservableField`] on which mutations you can
    /// subscribe, also you can subscribe on concrete mutation
    /// with [`ObservableField::when`] and [`ObservableField::when_eq`].
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
    S: Subscribable<D>,
{
    /// Creates new [`ObservableField`] with custom [`Subscribable`]
    /// implementation.
    pub fn new_with_custom(data: D, subs: S) -> Self {
        Self { data, subs }
    }
}

impl<D, S> ObservableField<D, S>
where
    D: 'static,
    S: Whenable<D>,
{
    /// Returns [`Future`] which will be resolved only on modification with
    /// which your `assert_fn` returned `true`.
    ///
    /// [`Future`]: std::future::Future
    pub fn when<F>(
        &self,
        assert_fn: F,
    ) -> LocalBoxFuture<'static, Result<(), Dropped>>
    where
        F: Fn(&D) -> bool + 'static,
    {
        if (assert_fn)(&self.data) {
            Box::pin(future::ok(()))
        } else {
            self.subs.when(Box::new(assert_fn))
        }
    }
}

impl<D, S> ObservableField<D, S>
where
    S: Subscribable<D>,
    D: Clone + 'static,
{
    /// Returns [`Stream`] into which underlying data updates will be emitted.
    ///
    /// [`Stream`]: futures::Stream
    pub fn subscribe(&self) -> LocalBoxStream<'static, D> {
        let data = self.data.clone();
        let subscription = self.subs.subscribe();

        Box::pin(stream::once(async move { data }).chain(subscription))
    }
}

impl<D, S> ObservableField<D, S>
where
    D: PartialEq + 'static,
    S: Whenable<D>,
{
    /// Returns [`Future`] which will be resolved only when data of this
    /// [`ObservableField`] will become equal to provided `should_be`.
    ///
    /// [`Future`]: std::future::Future
    pub fn when_eq(
        &self,
        should_be: D,
    ) -> LocalBoxFuture<'static, Result<(), Dropped>> {
        self.when(move |data| data == &should_be)
    }
}

impl<D, S> ObservableField<D, S>
where
    S: OnObservableFieldModification<D>,
    D: Clone + PartialEq,
{
    /// Returns [`MutObservableFieldGuard`] which can be mutably dereferenced to
    /// underlying data.
    ///
    /// If some mutation of data happened between calling
    /// [`ObservableField::borrow_mut`] and dropping of
    /// [`MutObservableFieldGuard`], then all subscribers of this
    /// [`ObservableField`] will be notified about this.
    ///
    /// Notification about mutation will be sent only if this field __really__
    /// changed. This will be checked with [`PartialEq`] implementation of
    /// underlying data.
    pub fn borrow_mut(&mut self) -> MutObservableFieldGuard<'_, D, S> {
        MutObservableFieldGuard {
            value_before_mutation: self.data.clone(),
            data: &mut self.data,
            subs: &mut self.subs,
        }
    }
}

/// With this trait you can catch all unique modification of a
/// [`ObservableField`].
pub trait OnObservableFieldModification<D> {
    /// This function will be called on every [`ObservableField`] modification.
    ///
    /// On this function call subsciber which implements
    /// [`OnObservableFieldModification`] should send a update to a [`Stream`]
    /// or resolve [`Future`].
    ///
    /// [`Stream`]: futures::Stream
    /// [`Future`]: std::future::Future
    fn on_modify(&mut self, data: &D);
}

/// With this trait you can implement [`ObservableField::subscribe`] functional
/// for some object.
pub trait Subscribable<D: 'static> {
    /// This function will be called on [`ObservableField::subscribe`].
    ///
    /// Should return [`LocalBoxStream`] to which will be sent data updates.
    fn subscribe(&self) -> LocalBoxStream<'static, D>;
}

/// Subscriber which implements [`Subscribable`] and [`Whenable`] in
/// [`Vec`].
///
/// This structure should be wrapped into [`Vec`].
pub enum UniversalSubscriber<D> {
    /// Subscriber for [`Whenable`].
    When {
        /// [`oneshot::Sender`] with which [`Whenable::when`]'s [`Future`] will
        /// be resolved.
        ///
        /// [`Future`]: std::future::Future
        sender: RefCell<Option<oneshot::Sender<()>>>,

        /// Function with which will be checked that [`Whenable::when`]'s
        /// [`Future`] should be resolved.
        ///
        /// [`Future`]: std::future::Future
        assert_fn: Box<dyn Fn(&D) -> bool>,
    },
    /// Subscriber for [`Subscribable`].
    Subscribe(mpsc::UnboundedSender<D>),
}

/// Error will be sent to all subscribers when this [`ObservableField`] is
/// dropped.
#[derive(Debug)]
pub struct Dropped;

impl From<oneshot::Canceled> for Dropped {
    fn from(_: oneshot::Canceled) -> Self {
        Self
    }
}

/// With this trait you can implement [`ObservableField::when`] and
/// [`ObservableField::when_eq`] functional for some object.
pub trait Whenable<D: 'static> {
    /// This function will be called on [`ObservableField::when`].
    ///
    /// Should return [`LocalBoxFuture`] to which will be sent `()` when
    /// provided `assert_fn` returns `true`.
    fn when(
        &self,
        assert_fn: Box<dyn Fn(&D) -> bool>,
    ) -> LocalBoxFuture<'static, Result<(), Dropped>>;
}

#[allow(clippy::use_self)]
impl<D: 'static> Whenable<D> for RefCell<Vec<UniversalSubscriber<D>>> {
    fn when(
        &self,
        assert_fn: Box<dyn Fn(&D) -> bool>,
    ) -> LocalBoxFuture<'static, Result<(), Dropped>> {
        let (tx, rx) = oneshot::channel();
        self.borrow_mut().push(UniversalSubscriber::When {
            sender: RefCell::new(Some(tx)),
            assert_fn,
        });

        Box::pin(async move { Ok(rx.await?) })
    }
}

impl<D: 'static> Subscribable<D> for RefCell<Vec<UniversalSubscriber<D>>> {
    fn subscribe(&self) -> LocalBoxStream<'static, D> {
        let (tx, rx) = mpsc::unbounded();
        self.borrow_mut().push(UniversalSubscriber::Subscribe(tx));

        Box::pin(rx)
    }
}

impl<D: Clone> OnObservableFieldModification<D>
    for RefCell<Vec<UniversalSubscriber<D>>>
{
    fn on_modify(&mut self, data: &D) {
        self.borrow_mut().retain(|sub| match sub {
            UniversalSubscriber::When { assert_fn, sender } => {
                if (assert_fn)(data) {
                    sender.borrow_mut().take().unwrap().send(()).ok();
                    false
                } else {
                    true
                }
            }
            UniversalSubscriber::Subscribe(sender) => {
                sender.unbounded_send(data.clone()).unwrap();
                true
            }
        });
    }
}

impl<D, S> Deref for ObservableField<D, S> {
    type Target = D;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<D, S> fmt::Debug for ObservableField<D, S>
where
    D: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "ObservableField {{ data: {:?} }}", self.data)
    }
}

impl<D, S> fmt::Display for ObservableField<D, S>
where
    D: fmt::Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}", self.data)
    }
}

/// Mutable [`ObservableField`] reference which you can get by calling
/// [`ObservableField::borrow_mut`].
///
/// When this object will be [`Drop`]ped check for modification will be
/// performed. If data was changed, then
/// [`OnObservableFieldModification::on_modify`] will be called.
pub struct MutObservableFieldGuard<'a, D, S>
where
    S: OnObservableFieldModification<D>,
    D: PartialEq,
{
    /// Data which stored by this [`ObservableField`].
    data: &'a mut D,

    /// Subscribers on [`ObservableField`]'s data mutations.
    subs: &'a mut S,

    /// Data which stored by this [`ObservableField`] before mutation.
    value_before_mutation: D,
}

impl<'a, D, S> Deref for MutObservableFieldGuard<'a, D, S>
where
    S: OnObservableFieldModification<D>,
    D: PartialEq,
{
    type Target = D;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'a, D, S> DerefMut for MutObservableFieldGuard<'a, D, S>
where
    S: OnObservableFieldModification<D>,
    D: PartialEq,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<'a, D, S> Drop for MutObservableFieldGuard<'a, D, S>
where
    S: OnObservableFieldModification<D>,
    D: PartialEq,
{
    fn drop(&mut self) {
        if self.data != &self.value_before_mutation {
            self.subs.on_modify(&self.data);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, time::Duration};

    use futures::{
        future::{self, Either, LocalBoxFuture},
        StreamExt,
    };
    use tokio::{task, time::delay_for};

    use crate::Observable;

    #[derive(Debug)]
    struct Timeout;

    async fn await_future_with_timeout<T>(
        fut: LocalBoxFuture<'_, T>,
        timeout: Duration,
    ) -> Result<T, Timeout> {
        let res = future::select(delay_for(timeout), fut).await;
        if let Either::Right((output, _)) = res {
            Ok(output)
        } else {
            Err(Timeout)
        }
    }

    #[tokio::test]
    async fn subscribe_sends_current_data() {
        let field = Observable::new(9i32);
        let current_data = field.subscribe().next().await.unwrap();
        assert_eq!(current_data, 9);
    }

    #[tokio::test]
    async fn when_eq_resolves_if_value_already_eq() {
        let field = Observable::new(9i32);
        field.when_eq(9i32).await.unwrap();
    }

    #[tokio::test]
    async fn when_eq_dont_resolves_if_value_is_not_eq() {
        let field = Observable::new(9i32);
        await_future_with_timeout(
            field.when_eq(0i32),
            Duration::from_millis(50),
        )
        .await
        .err()
        .unwrap();
    }

    #[tokio::test]
    async fn current_value_provided_into_assert_fn_on_when_call() {
        let field = Observable::new(9i32);

        await_future_with_timeout(
            field.when(|val| val == &9),
            Duration::from_millis(50),
        )
        .await
        .unwrap()
        .unwrap();
    }

    #[tokio::test]
    async fn value_updates_is_sended_to_subs() {
        task::LocalSet::new()
            .run_until(async move {
                let mut field = Observable::new(0i32);
                let mut subscription_on_changes = field.subscribe();

                task::spawn_local(async move {
                    for _ in 0..100 {
                        *field.borrow_mut() += 1;
                    }
                });
                loop {
                    if let Some(change) = subscription_on_changes.next().await {
                        if change == 100 {
                            break;
                        }
                    } else {
                        panic!("Stream ended too early!");
                    }
                }
            })
            .await;
    }

    #[tokio::test]
    async fn when_resolves_on_value_update() {
        task::LocalSet::new()
            .run_until(async move {
                let mut field = Observable::new(0i32);
                let subscription = field.when(|change| change == &100);

                task::spawn_local(async move {
                    for _ in 0..100 {
                        *field.borrow_mut() += 1;
                    }
                });

                await_future_with_timeout(
                    subscription,
                    Duration::from_millis(50),
                )
                .await
                .unwrap()
                .unwrap();
            })
            .await;
    }

    #[tokio::test]
    async fn when_eq_resolves_on_value_update() {
        task::LocalSet::new()
            .run_until(async move {
                let mut field = Observable::new(0i32);
                let subscription = field.when_eq(100);

                task::spawn_local(async move {
                    for _ in 0..100 {
                        *field.borrow_mut() += 1;
                    }
                });

                await_future_with_timeout(
                    subscription,
                    Duration::from_millis(50),
                )
                .await
                .unwrap()
                .unwrap();
            })
            .await;
    }

    #[tokio::test]
    async fn when_returns_dropped_error_on_drop() {
        let field = Observable::new(0i32);
        let subscription = field.when(|change| change == &100);
        drop(field);
        subscription.await.err().unwrap();
    }

    #[tokio::test]
    async fn when_eq_returns_dropped_error_on_drop() {
        let field = Observable::new(0i32);
        let subscription = field.when_eq(100);
        drop(field);
        subscription.await.err().unwrap();
    }

    #[tokio::test]
    async fn stream_ends_when_reactive_field_dropped() {
        let field = Observable::new(0i32);
        let subscription = field.subscribe();
        drop(field);
        assert!(subscription.skip(1).next().await.is_none());
    }

    #[tokio::test]
    async fn no_update_should_be_emitted_on_field_mutation() {
        let mut field = Observable::new(0i32);
        let subscription = field.subscribe();
        *field.borrow_mut() = 0;
        await_future_with_timeout(
            Box::pin(subscription.skip(1).next()),
            Duration::from_millis(50),
        )
        .await
        .err()
        .unwrap();
    }

    #[tokio::test]
    async fn only_last_update_should_be_send_to_the_subscribers() {
        let mut field = Observable::new(0i32);
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
        let field = RefCell::new(Observable::new(0i32));
        let subscription = field.borrow().when_eq(1);
        *field.borrow_mut().borrow_mut() = 1;
        await_future_with_timeout(
            Box::pin(subscription),
            Duration::from_millis(50),
        )
        .await
        .unwrap()
        .unwrap();
    }

    mod observable_cell {
        use super::*;

        use crate::ObservableCell;

        #[tokio::test]
        async fn subscription_works() {
            let field = ObservableCell::new(0i32);
            let subscription = field.subscribe();

            field.set(100i32);
            assert_eq!(subscription.skip(1).next().await.unwrap(), 100);
        }

        #[tokio::test]
        async fn when_works() {
            let field = ObservableCell::new(0i32);
            let when_will_be_greater_than_5 = field.when(|upd| upd > &5);

            field.set(6);
            await_future_with_timeout(
                Box::pin(when_will_be_greater_than_5),
                Duration::from_millis(50),
            )
            .await
            .unwrap()
            .unwrap();
        }

        #[tokio::test]
        async fn when_eq_works() {
            let field = ObservableCell::new(0i32);
            let when_will_be_5 = field.when_eq(5);

            field.set(5);
            await_future_with_timeout(
                Box::pin(when_will_be_5),
                Duration::from_millis(50),
            )
            .await
            .unwrap()
            .unwrap();
        }

        #[tokio::test]
        async fn only_initial_update_emitted() {
            let field = ObservableCell::new(0i32);
            let mut subscription = field.subscribe();
            assert_eq!(subscription.next().await.unwrap(), 0);

            await_future_with_timeout(
                Box::pin(subscription.next()),
                Duration::from_millis(10),
            )
            .await
            .unwrap_err();
        }

        #[tokio::test]
        async fn when_eq_never_resolves() {
            let field = ObservableCell::new(0i32);
            let when_will_be_5 = field.when_eq(5);

            await_future_with_timeout(
                Box::pin(when_will_be_5),
                Duration::from_millis(10),
            )
            .await
            .unwrap_err();
        }

        #[tokio::test]
        async fn data_mutates() {
            let field = ObservableCell::new(0i32);
            assert_eq!(*field.borrow(), 0);
            field.set(100500i32);
            assert_eq!(*field.borrow(), 100500i32);
        }

        #[tokio::test]
        async fn updates_emitted_on_replace() {
            let field = ObservableCell::new(0i32);
            let mut subscription = field.subscribe().skip(1);

            assert_eq!(field.replace(100), 0);
            assert_eq!(*field.borrow(), 100);

            assert_eq!(subscription.next().await.unwrap(), 100);
        }

        #[tokio::test]
        async fn when_works_on_replace() {
            let field = ObservableCell::new(0i32);
            let when_will_be_greater_than_5 = field.when(|upd| upd > &5);

            assert_eq!(field.replace(6), 0);

            await_future_with_timeout(
                Box::pin(when_will_be_greater_than_5),
                Duration::from_millis(50),
            )
            .await
            .unwrap()
            .unwrap();
        }

        #[tokio::test]
        async fn when_eq_works_on_replace() {
            let field = ObservableCell::new(0i32);
            let when_will_be_5 = field.when_eq(5);

            assert_eq!(field.replace(5), 0);

            await_future_with_timeout(
                Box::pin(when_will_be_5),
                Duration::from_millis(50),
            )
            .await
            .unwrap()
            .unwrap();
        }

        #[tokio::test]
        async fn get_works() {
            let field = ObservableCell::new(0i32);
            assert_eq!(field.get(), 0);

            field.set(5);
            assert_eq!(field.get(), 5);

            assert_eq!(field.replace(10), 5);
            assert_eq!(field.get(), 10);
        }

        #[tokio::test]
        async fn emits_changes_on_mutate() {
            let field = ObservableCell::new(0i32);
            let mut subscription = field.subscribe().skip(1);

            field.mutate(|mut data| *data = 100);
            assert_eq!(subscription.next().await.unwrap(), 100);
        }

        #[tokio::test]
        async fn when_works_with_mutate() {
            let field = ObservableCell::new(0i32);
            let when_will_be_5 = field.when_eq(5);

            field.mutate(|mut data| *data = 5);
            await_future_with_timeout(
                Box::pin(when_will_be_5),
                Duration::from_millis(50),
            )
            .await
            .unwrap()
            .unwrap();
        }
    }
}
