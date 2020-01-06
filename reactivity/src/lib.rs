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
{
    pub fn unsafe_borrow_mut(&mut self) -> UnsafeMutReactiveField<'_, T, S> {
        UnsafeMutReactiveField {
            data: &mut self.data,
            subs: &self.subs,
        }
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

pub struct UnsafeMutReactiveField<'a, T, S>
where
    S: OnReactiveFieldModification<T>,
{
    data: &'a mut T,
    subs: &'a S,
}

impl<'a, T, S> Deref for UnsafeMutReactiveField<'a, T, S>
where
    S: OnReactiveFieldModification<T>,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'a, T, S> DerefMut for UnsafeMutReactiveField<'a, T, S>
where
    S: OnReactiveFieldModification<T>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<'a, T, S> Drop for UnsafeMutReactiveField<'a, T, S>
where
    S: OnReactiveFieldModification<T>,
{
    fn drop(&mut self) {
        self.subs.on_modify(&self.data);
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

#[cfg(test)]
mod test {
    use futures::StreamExt;

    use super::*;

    #[derive(Clone, Debug)]
    struct Foo {
        bar: u32,
    }

    impl Foo {
        pub fn new() -> Self {
            Self { bar: 0 }
        }

        pub fn bump(&mut self) {
            self.bar += 1;
        }

        pub fn get_count(&self) -> u32 {
            self.bar
        }
    }

    struct Bar {
        bar: u32,
    }

    impl Bar {
        pub fn new() -> Self {
            Self { bar: 0 }
        }

        pub fn bump(&mut self) {
            self.bar += 1;
        }
    }

    impl OnReactiveFieldModification<Bar> for DefaultSubscribable<u32> {
        fn on_modify(&self, data: &Bar) {
            let bar = data.bar;
            self.iter()
                .filter(|sub| !sub.is_closed())
                .for_each(|sub| sub.unbounded_send(bar).unwrap());
        }
    }

    #[tokio::test]
    async fn mut_borrow() {
        let mut foo = ReactiveField::new(Foo::new());
        let mut stream = foo.subscribe();
        foo.unsafe_borrow_mut().bump();
        // panic!("{:?}", stream.next().await.unwrap())
    }

    #[tokio::test]
    async fn bar() {
        let mut bar: CustomReactiveField<Bar, u32> =
            ReactiveField::new_with_custom(Bar::new(), Vec::new());
        let mut stream = bar.subscribe();
        bar.unsafe_borrow_mut().bump();
        panic!("{}", stream.next().await.unwrap())
    }

    struct DifferentFields {
        custom_output_field: CustomReactiveField<Bar, u32>,
        same_out_field: DefaultReactiveField<Foo>,
    }

    impl DifferentFields {
        pub fn new() -> Self {
            Self {
                custom_output_field: ReactiveField::new_with_custom(
                    Bar::new(),
                    Vec::new(),
                ),
                same_out_field: ReactiveField::new(Foo::new()),
            }
        }
    }

    #[tokio::test]
    async fn different_fields() {}
}
