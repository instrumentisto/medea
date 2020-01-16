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
pub type Reactive<D> = ReactiveField<D, RefCell<Vec<UniversalSubscriber<D>>>>;

/// A reactive cell which will emit all modification to the subscribers.
///
/// You can subscribe to this field modifications with
/// [`ReactiveField::subscribe`].
///
/// If you want to get [`Future`] which will be resolved only when data of this
/// field will become equal to some data, you can use [`ReactiveField::when`] or
/// [`ReactiveField::when_eq`].
pub struct ReactiveField<D, S> {
    /// Data which stored by this [`ReactiveField`].
    data: D,

    /// Subscribers on [`ReactiveField`]'s data mutations.
    subs: S,
}

impl<D, S> fmt::Debug for ReactiveField<D, S>
where
    D: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "ReactiveField {{ data: {:?} }}", self.data)
    }
}

impl<D> ReactiveField<D, RefCell<Vec<UniversalSubscriber<D>>>>
where
    D: 'static,
{
    /// Returns new [`ReactiveField`] on which mutations you can
    /// [`ReactiveSubscribe`], also you can subscribe on concrete mutation with
    /// [`ReactiveField::when`] and [`ReactiveField::when_eq`].
    pub fn new(data: D) -> Self {
        Self {
            data,
            subs: RefCell::new(Vec::new()),
        }
    }
}

impl<D, S> ReactiveField<D, S>
where
    D: 'static,
    S: Subscribable<D>,
{
    /// Creates new [`ReactiveField`] with custom [`Subscribable`]
    /// implementation.
    pub fn new_with_custom(data: D, subs: S) -> Self {
        Self { data, subs }
    }
}

impl<D, S> ReactiveField<D, S>
where
    D: 'static,
    S: Whenable<D>,
{
    /// Returns [`Future`] which will be resolved only on modification with
    /// which your `assert_fn` returned `true`.
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

impl<D, S> ReactiveField<D, S>
where
    S: Subscribable<D>,
    D: Clone + 'static,
{
    pub fn subscribe(&self) -> LocalBoxStream<'static, D> {
        let data = self.data.clone();
        let subscription = self.subs.subscribe();

        Box::pin(futures::stream::once(async move { data }).chain(subscription))
    }
}

impl<D, S> ReactiveField<D, S>
where
    D: Eq + 'static,
    S: Whenable<D>,
{
    /// Returns [`Future`] which will be resolved only when data of this
    /// [`ReactiveField`] will become equal to provided `should_be`.
    pub fn when_eq(
        &self,
        should_be: D,
    ) -> LocalBoxFuture<'static, Result<(), Dropped>> {
        self.when(move |data| data == &should_be)
    }
}

impl<D, S> ReactiveField<D, S>
where
    S: OnReactiveFieldModification<D>,
    D: Clone + Eq,
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
    pub fn borrow_mut(&mut self) -> MutReactiveFieldGuard<'_, D, S> {
        MutReactiveFieldGuard {
            value_before_mutation: self.data.clone(),
            data: &mut self.data,
            subs: &mut self.subs,
        }
    }
}

pub trait OnReactiveFieldModification<D> {
    /// This function will be called on every [`ReactiveField`] modification.
    ///
    /// On this function call subsciber which implements
    /// [`OnReactiveFieldModification`] should send a update to a [`Stream`]
    /// or resolve [`Future`].
    fn on_modify(&mut self, data: &D);
}

pub trait Subscribable<D: 'static> {
    /// This function will be called on [`ReactiveField::subscribe`].
    ///
    /// Should return [`LocalBoxStream`] to which will be sent data updates.
    fn subscribe(&self) -> LocalBoxStream<'static, D>;
}

/// Subscriber which implements [`Subscribable`] and [`SubscribableOnce`] in
/// [`Vec`].
///
/// This structure should be wrapped into [`Vec`].
pub enum UniversalSubscriber<D> {
    When {
        sender: RefCell<Option<oneshot::Sender<()>>>,
        assert_fn: Box<dyn Fn(&D) -> bool>,
    },
    All(mpsc::UnboundedSender<D>),
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

pub trait Whenable<D: 'static> {
    /// This function will be called on [`ReactiveField::when`].
    ///
    /// Should return [`LocalBoxFuture`] to which will be sent `()` when
    /// provided `assert_fn` returns `true`.
    fn when(
        &self,
        assert_fn: Box<dyn Fn(&D) -> bool>,
    ) -> LocalBoxFuture<'static, Result<(), Dropped>>;
}

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
        self.borrow_mut().push(UniversalSubscriber::All(tx));

        Box::pin(rx)
    }
}

impl<D: Clone> OnReactiveFieldModification<D> for RefCell<Vec<UniversalSubscriber<D>>> {
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
            UniversalSubscriber::All(sender) => {
                sender.unbounded_send(data.clone()).unwrap();
                true
            }
        });
    }
}

impl<D, S> Deref for ReactiveField<D, S> {
    type Target = D;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

pub struct MutReactiveFieldGuard<'a, D, S>
where
    S: OnReactiveFieldModification<D>,
    D: Eq,
{
    data: &'a mut D,
    subs: &'a mut S,
    value_before_mutation: D,
}

impl<'a, D, S> Deref for MutReactiveFieldGuard<'a, D, S>
where
    S: OnReactiveFieldModification<D>,
    D: Eq,
{
    type Target = D;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'a, D, S> DerefMut for MutReactiveFieldGuard<'a, D, S>
where
    S: OnReactiveFieldModification<D>,
    D: Eq,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<'a, D, S> Drop for MutReactiveFieldGuard<'a, D, S>
where
    S: OnReactiveFieldModification<D>,
    D: Eq,
{
    fn drop(&mut self) {
        if self.data != &self.value_before_mutation {
            self.subs.on_modify(&self.data);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures::{
        future::{self, Either, LocalBoxFuture},
        StreamExt,
    };
    use tokio::{task, time::delay_for};

    use crate::Reactive;

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
        let mut field = Reactive::new(9i32);
        let current_data = field.subscribe().next().await.unwrap();
        assert_eq!(current_data, 9);
    }

    #[tokio::test]
    async fn when_eq_resolves_if_value_already_eq() {
        let mut field = Reactive::new(9i32);
        field.when_eq(9i32).await.unwrap();
    }

    #[tokio::test]
    async fn when_eq_dont_resolves_if_value_is_not_eq() {
        let mut field = Reactive::new(9i32);
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
        let mut field = Reactive::new(9i32);

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
                let mut field = Reactive::new(0i32);
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
                let mut field = Reactive::new(0i32);
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
                let mut field = Reactive::new(0i32);
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
        let mut field = Reactive::new(0i32);
        let subscription = field.when(|change| change == &100);
        std::mem::drop(field);
        subscription.await.err().unwrap();
    }

    #[tokio::test]
    async fn when_eq_returns_dropped_error_on_drop() {
        let mut field = Reactive::new(0i32);
        let subscription = field.when_eq(100);
        std::mem::drop(field);
        subscription.await.err().unwrap();
    }

    #[tokio::test]
    async fn stream_ends_when_reactive_field_dropped() {
        let mut field = Reactive::new(0i32);
        let subscription = field.subscribe();
        std::mem::drop(field);
        assert!(subscription.skip(1).next().await.is_none());
    }

    #[tokio::test]
    async fn no_update_should_be_emitted_on_field_mutation() {
        let mut field = Reactive::new(0i32);
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
        let mut field = Reactive::new(0i32);
        let subscription = field.subscribe();
        let mut field_mut_guard = field.borrow_mut();
        *field_mut_guard = 100;
        *field_mut_guard = 200;
        *field_mut_guard = 300;
        std::mem::drop(field_mut_guard);
        assert_eq!(subscription.skip(1).next().await.unwrap(), 300);
    }
}
