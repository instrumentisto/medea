use std::rc::Rc;

use super::{
    global, mutex_lock, transmute, waker_ref, Box, BoxFuture, Context,
    ExternTask, InternTask, Mutex, Poll, TaskWake, Wake, WrappedUserData,
};

pub(crate) type BoxedPoll = Box<dyn FnMut() -> bool>;

/// Create task for polling specified future by external event loop
pub fn task_wrap<T: 'static>(
    future: BoxFuture<'static, T>,
    data: ExternTask,
) -> InternTask {
    let task = Task::new(future, data);

    let poll = Box::new(move || task.poll()) as BoxedPoll;

    Box::into_raw(Box::new(poll)) as _
}

pub(crate) struct Task<T> {
    future: Mutex<BoxFuture<'static, T>>,
    data: WrappedUserData,
}

pub(crate) fn wake(data: InternTask) {
    let task_wake: TaskWake = unsafe { transmute(global::TASK_WAKE) };

    task_wake(data);
}

impl<T> Wake for Task<T> {
    fn wake_by_ref(arc_self: &Rc<Self>) {
        wake(*arc_self.data);
    }
}

impl<T> Task<T> {
    pub fn new(future: BoxFuture<'static, T>, data: ExternTask) -> Rc<Self> {
        let future = Mutex::new(future);
        let data = data.into();

        Rc::new(Self { future, data })
    }

    pub fn poll(self: &Rc<Self>) -> bool {
        let mut future = mutex_lock(&self.future);
        let waker = waker_ref(&self);
        let context = &mut Context::from_waker(&*waker);

        if let Poll::Pending = future.as_mut().poll(context) {
            true
        } else {
            false
        }
    }
}
