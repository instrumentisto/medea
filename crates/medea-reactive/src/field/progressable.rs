use futures::{
    future::LocalBoxFuture, stream::LocalBoxStream,
    StreamExt as _,
};

use crate::{
    progressable::{ProgressableManager, ProgressableObservableValue},
};

use super::Observable;

#[derive(Debug)]
pub struct ProgressableObservable<D> {
    data: Observable<D>,
    progressable_manager: ProgressableManager,
}

impl<D> ProgressableObservable<D>
where
    D: 'static,
{
    pub fn new(data: D) -> Self {
        let mut data = Observable::new(data);
        let progressable_manager = ProgressableManager::new();
        data.add_modify_callback({
            let progressable_manager = progressable_manager.clone();
            move |sub_count| {
                progressable_manager.incr_processors_count(sub_count as u32);
            }
        });

        Self {
            data,
            progressable_manager,
        }
    }
}

impl<D> ProgressableObservable<D> {
    pub fn when_all_processed(&self) -> LocalBoxFuture<'static, ()> {
        self.progressable_manager.when_all_processed()
    }
}

impl<D> ProgressableObservable<D>
where
    D: Clone + 'static,
{
    pub fn subscribe(
        &self,
    ) -> LocalBoxStream<'static, ProgressableObservableValue<D>> {
        self.progressable_manager.incr_processors_count(1);
        Box::pin(self.data.subscribe().map({
            let progressable_manager = self.progressable_manager.clone();
            move |v| progressable_manager.new_value(v)
        }))
    }
}

impl<D> ProgressableObservable<D>
where
    D: Clone + PartialEq,
{
    #[inline]
    pub fn borrow_mut(
        &mut self,
    ) -> super::MutObservableFieldGuard<'_, D, super::DefaultSubscribers<D>>
    {
        self.data.borrow_mut()
    }
}
