#![feature(test)]

use std::{
    fmt,
    fmt::{Debug, Error, Formatter},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    pin::Pin,
};

use futures::{channel::{mpsc, oneshot}, future::LocalBoxFuture, stream::LocalBoxStream, Future, Stream, StreamExt as _, future};
use std::cell::RefCell;
use futures::future::Either;

pub type DefaultSubscribable<T> = Vec<mpsc::UnboundedSender<T>>;

/// [`ReactiveField`] with which you can only subscribe on changes [`Stream`].
pub type DefaultReactiveField<T> = ReactiveField<T, DefaultSubscribable<T>, T>;

/// [`ReactiveField`] with custom subscriber.
pub type CustomReactiveField<T, O> =
    ReactiveField<T, DefaultSubscribable<O>, O>;

/// [`ReactiveField`] with which you can only subscribe on concrete change with
/// [`ReactiveField::when`] and [`ReactiveField::when_eq`].
pub type OnceReactiveField<T> = ReactiveField<T, Vec<SubscriberOnce<T>>, T>;

/// [`ReactiveField`] to which you can subscribe on all changes and only on
/// concrete change (with [`ReactiveField::when`] or
/// [`ReactiveField::when_eq`]).
pub type OnceAndManyReactiveField<T> =
    ReactiveField<T, Vec<UniversalSubscriber<T>>, T>;

/// A reactive cell which will emit all modification to the subscribers.
///
/// You can subscribe to this field modifications with
/// [`ReactiveField::subscribe`].
///
/// If you want to get [`Future`] which will be resolved only when data of this
/// field will become equal to some data, you can use [`ReactiveField::when`] or
/// [`ReactiveField::when_eq`].
pub struct ReactiveField<T, S, O> {
    /// Data which stored by this [`ReactiveField`].
    data: T,

    /// Subscribers on [`ReactiveField`]'s data mutations.
    subs: S,

    /// Output of [`ReactiveField::subscribe`] [`Stream`].
    _subscribable_output: PhantomData<O>,
}

impl<T, S, O> fmt::Debug for ReactiveField<T, S, O>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "ReactiveField {{ data: {:?} }}", self.data)
    }
}

impl<T> ReactiveField<T, Vec<mpsc::UnboundedSender<T>>, T>
where
    T: 'static,
{
    /// Returns new [`ReactiveField`] on which modification you can only
    /// [`ReactiveField::subscribe`].
    pub fn new(data: T) -> Self {
        Self {
            data,
            subs: Vec::new(),
            _subscribable_output: PhantomData::default(),
        }
    }
}

impl<T> ReactiveField<T, Vec<SubscriberOnce<T>>, T>
where
    T: 'static,
{
    /// Returns new [`ReactiveField`] on which mutations you can't
    /// [`ReactiveField::subscribe`], but you can subscribe on concrete
    /// mutation with [`ReactiveField::when`] and [`ReactiveField::when_eq`].
    pub fn new(data: T) -> Self {
        Self {
            data,
            subs: Vec::new(),
            _subscribable_output: PhantomData::default(),
        }
    }
}

impl<T> ReactiveField<T, Vec<UniversalSubscriber<T>>, T>
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
            _subscribable_output: PhantomData::default(),
        }
    }
}

impl<T, S, O> ReactiveField<T, S, O>
where
    O: 'static,
    S: Subscribable<O>,
{
    /// Creates new [`ReactiveField`] with custom [`Subscribable`]
    /// implementation.
    pub fn new_with_custom(data: T, subs: S) -> Self {
        Self {
            data,
            subs,
            _subscribable_output: PhantomData::default(),
        }
    }
}

impl<T, S, O> ReactiveField<T, S, O>
where
    T: 'static,
    O: 'static,
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
            self.subs.subscribe_once(Box::new(assert_fn))
        }
    }
}

impl<T, S, O> ReactiveField<T, S, O>
where
    O: 'static,
    S: Subscribable<O>,
{
    pub fn subscribe(&mut self) -> LocalBoxStream<'static, O> {
        self.subs.subscribe()
    }
}

impl<T, S, O> ReactiveField<T, S, O>
where
    T: Eq + 'static,
    O: 'static,
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

impl<T, S, O> ReactiveField<T, S, O>
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

/// Subscriber which implements only [`SubscribableOnce`].
///
/// This structure should be wrapped into [`Vec`].
pub struct SubscriberOnce<T> {
    pub sender: RefCell<Option<oneshot::Sender<()>>>,
    pub assert_fn: Box<dyn Fn(&T) -> bool>,
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
    fn subscribe_once(
        &mut self,
        assert_fn: Box<dyn Fn(&T) -> bool>,
    ) -> LocalBoxFuture<'static, Result<(), Dropped>>;
}

impl<T: 'static> SubscribableOnce<T> for Vec<UniversalSubscriber<T>> {
    fn subscribe_once(
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
                    sender.borrow_mut().take().unwrap().send(());
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

impl<T: 'static> SubscribableOnce<T> for Vec<SubscriberOnce<T>> {
    fn subscribe_once(
        &mut self,
        assert_fn: Box<dyn Fn(&T) -> bool>,
    ) -> LocalBoxFuture<'static, Result<(), Dropped>> {
        let (tx, rx) = oneshot::channel();
        self.push(SubscriberOnce {
            sender: RefCell::new(Some(tx)),
            assert_fn,
        });

        Box::pin(async move { Ok(rx.await?) })
    }
}

impl<T> OnReactiveFieldModification<T> for Vec<SubscriberOnce<T>>
where
    T: Clone,
{
    fn on_modify(&mut self, data: &T) {
        self.retain(|sub| {
            if (sub.assert_fn)(data) {
                sub.sender.borrow_mut().take().unwrap().send(());
                true
            } else {
                false
            }
        });
    }
}

impl<T: 'static> Subscribable<T> for Vec<mpsc::UnboundedSender<T>> {
    fn subscribe(&mut self) -> LocalBoxStream<'static, T> {
        let (tx, rx) = mpsc::unbounded();
        self.push(tx);

        Box::pin(rx)
    }
}

impl<T> OnReactiveFieldModification<T> for Vec<mpsc::UnboundedSender<T>>
where
    T: Clone,
{
    fn on_modify(&mut self, data: &T) {
        self.iter()
            .filter(|sub| !sub.is_closed())
            .for_each(|sub| sub.unbounded_send(data.clone()).unwrap());
    }
}

impl<T, S, O> Deref for ReactiveField<T, S, O> {
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

#[cfg(test)]
mod test {
    extern crate test as std_test;

    use futures::{StreamExt, TryFutureExt};
    use std_test::Bencher;

    use super::*;
    //    use crate::alexlapa_reactivity::ObservableField;
    use futures_signals::signal::SignalExt;

    const MUTATE_COUNT: i32 = 10_000;

    #[bench]
    fn this_primitive(b: &mut Bencher) {
        b.iter(|| {
            futures::executor::block_on(async move {
                let mut x = OnceAndManyReactiveField::new(1i32);
                let wait_for_1001 = x.when_eq(MUTATE_COUNT);
                for _ in 0..MUTATE_COUNT {
                    *x.borrow_mut() += 1;
                }
                wait_for_1001.await;
            });
        });
    }

    // #[bench]
    // fn alexlapa_primitive(b: &mut Bencher) {
    // b.iter(|| {
    // futures::executor::block_on(async {
    // let mut x = ObservableField::new(1i32);
    // let wait_for_1001 = x.once_when_eq(MUTATE_COUNT);
    // for _ in 0..MUTATE_COUNT {
    // let qq = x.data + 1;
    // x.set(qq);
    // }
    // wait_for_1001.await.expect("Nope");
    // })
    // })
    // }

    #[bench]
    fn futures_signals_primitive(b: &mut Bencher) {
        use futures_signals::signal::Mutable;

        b.iter(|| {
            futures::executor::block_on(async {
                let mut x = Mutable::new(1i32);
                let mut signal = x.signal().to_stream();
                let wait_for_1001 = async move {
                    while let Some(update) = signal.next().await {
                        if update > MUTATE_COUNT {
                            break;
                        }
                    }
                };

                for _ in 0..MUTATE_COUNT {
                    *x.lock_mut() += 1;
                }
                wait_for_1001.await;
            })
        })
    }
}
