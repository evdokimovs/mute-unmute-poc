#![feature(test)]
#![feature(drain_filter)]

use std::{
    fmt,
    fmt::{Debug, Error, Formatter},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use futures::{
    channel::mpsc, future::LocalBoxFuture, stream::LocalBoxStream,
    StreamExt as _,
};

pub type DefaultSubscribable<T> = Vec<mpsc::UnboundedSender<T>>;
pub type DefaultReactiveField<T> = ReactiveField<T, DefaultSubscribable<T>, T>;
pub type CustomReactiveField<T, O> =
    ReactiveField<T, DefaultSubscribable<O>, O>;

pub struct ReactiveField<T, S, O> {
    data: T,
    subs: S,
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
    O: 'static,
    S: Subscribable<O>,
{
    pub fn subscribe(&mut self) -> LocalBoxStream<'static, O> {
        self.subs.subscribe()
    }

    pub fn when<F>(
        &mut self,
        resolver: F,
    ) -> LocalBoxFuture<'static, Result<(), ()>>
    where
        F: Fn(O) -> bool + 'static,
    {
        let mut changes_stream = self.subs.subscribe();
        Box::pin(async move {
            while let Some(on_data_change) = changes_stream.next().await {
                if (resolver)(on_data_change) {
                    return Ok(());
                }
            }
            Err(())
        })
    }
}

impl<T, S, O> ReactiveField<T, S, O>
where
    O: Eq + 'static,
    S: Subscribable<O>,
{
    pub fn when_eq(
        &mut self,
        should_be: O,
    ) -> LocalBoxFuture<'static, Result<(), ()>> {
        self.when(move |data| data == should_be)
    }
}

impl<T, S, O> ReactiveField<T, S, O>
where
    S: OnReactiveFieldModification<T>,
    T: Clone + Eq,
{
    pub fn borrow_mut(&mut self) -> SafeMutReactiveField<'_, T, S> {
        SafeMutReactiveField {
            value_before_mutation: self.data.clone(),
            data: &mut self.data,
            subs: &self.subs,
        }
    }
}

pub trait OnReactiveFieldModification<T> {
    fn on_modify(&self, data: &T);
}

pub trait Subscribable<T: 'static> {
    fn subscribe(&mut self) -> LocalBoxStream<'static, T>;
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
    fn on_modify(&self, data: &T) {
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

pub struct SafeMutReactiveField<'a, T, S>
where
    S: OnReactiveFieldModification<T>,
    T: Eq,
{
    data: &'a mut T,
    subs: &'a S,
    value_before_mutation: T,
}

impl<'a, T, S> Deref for SafeMutReactiveField<'a, T, S>
where
    S: OnReactiveFieldModification<T>,
    T: Eq,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'a, T, S> DerefMut for SafeMutReactiveField<'a, T, S>
where
    S: OnReactiveFieldModification<T>,
    T: Eq,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<'a, T, S> Drop for SafeMutReactiveField<'a, T, S>
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

mod alexlapa_reactivity {

    use futures::{
        channel::{mpsc, oneshot},
        executor,
        future::{FutureExt, LocalBoxFuture, TryFutureExt},
        stream::LocalBoxStream,
    };
    use std::cell::RefCell;
    use tokio::prelude::*;

    pub struct Dropped;

    enum AssertType<T: PartialEq> {
        Predicate(Box<dyn Fn(&T) -> bool>),
        Val(T),
    }

    impl<T: PartialEq> PartialEq<T> for AssertType<T> {
        fn eq(&self, other: &T) -> bool {
            match &self {
                Self::Predicate(predicate) => predicate(other),
                Self::Val(val) => val == other,
            }
        }
    }

    enum Subscriber<T: PartialEq> {
        Flux(AssertType<T>, mpsc::Sender<()>),
        Mono(AssertType<T>, Option<oneshot::Sender<()>>),
    }

    pub struct ObservableField<T: PartialEq> {
        pub data: T,
        subs: RefCell<Vec<Subscriber<T>>>,
    }

    impl<T: PartialEq> ObservableField<T> {
        pub fn new(inner: T) -> Self {
            Self {
                data: inner,
                subs: RefCell::new(vec![]),
            }
        }

        fn when_eq(&self, val: T) -> LocalBoxStream<'static, ()> {
            unimplemented!()
        }

        fn when(
            &self,
            f: Box<dyn Fn(&T) -> bool>,
        ) -> LocalBoxStream<'static, ()> {
            unimplemented!()
        }

        pub fn once_when_eq(
            &self,
            val: T,
        ) -> LocalBoxFuture<'static, Result<(), Dropped>> {
            let (tx, rx) = oneshot::channel();
            self.subs
                .borrow_mut()
                .push(Subscriber::Mono(AssertType::Val(val), Some(tx)));
            rx.map_err(|_| Dropped).boxed()
        }

        pub fn set(&mut self, val: T) {
            self.data = val;

            let mut subs = self.subs.borrow_mut();

            subs.drain_filter(|sub| match sub {
                Subscriber::Flux(ref assert, sender) => {
                    if !sender.is_closed() {
                        if assert == &self.data {
                            let _ = sender.try_send(());
                        }
                        false
                    } else {
                        true
                    }
                }
                Subscriber::Mono(ref assert, sender) => {
                    if assert == &self.data {
                        let sender = sender.take().expect("MEh");
                        if !sender.is_canceled() {
                            if assert == &self.data {
                                let _ = sender.send(());
                                true
                            } else {
                                false
                            }
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod test {
    extern crate test as std_test;

    use futures::StreamExt;
    use std_test::Bencher;

    use super::*;
    use crate::alexlapa_reactivity::ObservableField;

    #[bench]
    fn this_primitive(b: &mut Bencher) {
        b.iter(|| {
            futures::executor::block_on(async {
                let mut x = DefaultReactiveField::new(1i32);
                let wait_for_1001 = x.when_eq(1001);
                for _ in 0..1000 {
                    *x.borrow_mut() += 1;
                }
                wait_for_1001.await;
            });
        });
    }

    #[bench]
    fn alexlapa_primitive(b: &mut Bencher) {
        b.iter(|| {
            futures::executor::block_on(async {
                let mut x = ObservableField::new(1i32);
                let wait_for_1001 = x.once_when_eq(1001);
                for _ in 0..1000 {
                    let qq = x.data + 1;
                    x.set(qq);
                }
                wait_for_1001.await;
            })
        })
    }
}
