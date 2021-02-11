#![allow(dead_code)]

use futures::{
    channel::mpsc,
    future::{BoxFuture, Future},
    stream::StreamExt,
};
use once_cell::sync::Lazy;

pub static RUST_EXEC_CONTEXT: Lazy<RustContext> = Lazy::new(RustContext::new);

pub struct RustContext {
    tx: mpsc::UnboundedSender<Task>,
}

enum Task {
    Blocking(Box<dyn Fn() + Send>),
    Async(BoxFuture<'static, ()>),
}

enum TaskResult<T> {
    Ok(T),
    Panic(String),
}

impl RustContext {
    pub fn new() -> Self {
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let _runtime = std::thread::Builder::new()
            .name(String::from("jason-worker"))
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async move {
                    while let Some(task) = rx.next().await {
                        tokio::spawn(async move {
                            match task {
                                Task::Blocking(task) => {
                                    task();
                                }
                                Task::Async(task) => {
                                    task.await;
                                }
                            }
                        });
                    }
                });
            });

        Self { tx }
    }

    // pub fn spawn_async<T>(&self, task: T) -> oneshot::Receiver<T::Output>
    // where
    //     T: Future + Send + 'static,
    //     T::Output: Send,
    // {
    //     let (tx, rx) = oneshot::channel();
    //     self.tx
    //         .unbounded_send(Task::Async(Box::pin(async {
    //             if !tx.is_canceled() {
    //                 let _ = tx.send(task.await);
    //             }
    //         })))
    //         .unwrap();
    //     rx
    // }

    pub fn spawn_async<T>(&self, task: T) -> T::Output
    where
        T: Future + Send + 'static,
        T::Output: Send,
    {
        let (tx, rx) = std::sync::mpsc::channel();
        self.tx
            .unbounded_send(Task::Async(Box::pin(async move {
                let result = task.await;
                let _ = tx.send(result);
            })))
            .unwrap();
        rx.recv().unwrap()
    }

    pub fn spawn_blocking<T, R>(&self, task: T) -> R
    where
        T: Fn() -> R + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = std::sync::mpsc::channel();
        self.tx
            .unbounded_send(Task::Blocking(Box::new(move || {
                let _ = tx.send(task());
            })))
            .unwrap();
        rx.recv().unwrap()
    }
}
