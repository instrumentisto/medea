//! Utils functions and structures for the testing purposes.

mod freezeable_delay;

use std::{cell::RefCell, rc::Rc, time::Duration};

use futures::{channel::mpsc, Stream};
use medea_jason::utils::delay_for;

/// This function will release async context, so another spawned functions
/// can do something.
///
/// For example, some [`Stream`] should be pulled after some
/// sync function call. To do this you can call needed sync function
/// and after it this function.
pub async fn release_async_runtime() {
    delay_for(Duration::from_micros(0).into()).await;
}

/// This structure can be used to mock [`Stream`]s.
#[derive(Clone)]
pub struct StreamMock<T> {
    /// [`mpsc::UnboundedSender`]s for the all [`Stream`]s of this
    /// [`StreamMock`].
    senders: Rc<RefCell<Vec<mpsc::UnboundedSender<T>>>>,

    /// First message which will be sent to the newly created [`Stream`].
    ///
    /// If this function will return `None` then no message will be sent.
    first_msg: Rc<dyn Fn() -> Option<T>>,
}

impl<T> StreamMock<T>
where
    T: Clone,
{
    /// Creates new [`StreamMock`].
    ///
    /// Result of the provided function will be sent to the newly created
    /// [`Stream`]s.
    ///
    /// If result of the provided function will be `None` then no message will
    /// be sent.
    pub fn new(first_msg: Rc<dyn Fn() -> Option<T>>) -> Self {
        Self {
            senders: Rc::new(RefCell::new(Vec::new())),
            first_msg,
        }
    }

    /// Returns [`Stream`] to which will be sent messages.
    ///
    /// If some message should be sent to the newly created [`Stream`] then this
    /// will be performed in this function.
    pub fn get_stream(&self) -> impl Stream<Item = T> {
        let (tx, rx) = mpsc::unbounded();
        if let Some(msg) = (self.first_msg)() {
            let _ = tx.unbounded_send(msg);
        }
        self.senders.borrow_mut().push(tx);

        rx
    }

    /// Sends message to the all [`Stream`]s of this [`StreamMock`].
    pub fn send(&self, msg: T) {
        let inner = self.senders.borrow();
        for sender in inner.iter() {
            let _ = sender.unbounded_send(msg.clone());
        }
    }
}
