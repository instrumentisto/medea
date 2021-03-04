use std::sync::mpsc as std_mpsc;

use crate::jni::{
    util::{JNIEnv, JavaVM},
    AsyncTaskCallback,
};

use futures::{
    channel::mpsc as fut_mpsc, future::BoxFuture, stream::StreamExt, Future,
};

#[derive(Clone)]
pub struct RustExecutor(fut_mpsc::UnboundedSender<Task>);

impl RustExecutor {
    pub fn new() -> Self {
        let (tx, mut rx) = fut_mpsc::unbounded();
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
                            };
                        });
                    }
                });
            });

        Self(tx)
    }

    pub fn really_spawn_async<T>(&self, task: T, cb: AsyncTaskCallback<()>)
    where
        T: Future + Send + 'static,
    {
        self.0
            .unbounded_send(Task::Async(Box::pin(async {
                task.await; // TODO: pass result to callback
                cb.resolve(());
            })))
            .unwrap();
    }

    pub fn spawn_async<T>(&self, task: T) -> T::Output
    where
        T: Future + Send + 'static,
        T::Output: Send,
    {
        let (tx, rx) = std::sync::mpsc::channel();
        self.0
            .unbounded_send(Task::Async(Box::pin(async move {
                let result = task.await;
                let _ = tx.send(result);
            })))
            .unwrap();
        rx.recv().unwrap()
    }

    pub fn blocking_exec<T, R>(&self, task: T) -> R
    where
        T: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = std::sync::mpsc::channel();
        self.0
            .unbounded_send(Task::Blocking(Box::new(move || {
                let result = task();
                let _ = tx.send(result);
            })))
            .unwrap();
        rx.recv().unwrap()
    }
}

enum Task {
    Blocking(Box<dyn FnOnce() + Send>),
    Async(BoxFuture<'static, ()>),
}

// enum TaskResult<T> {
//     Ok(T),
//     Panic(String),
// }

// Executors.newSingleThreadExecutor()
#[derive(Clone)]
pub struct JavaExecutor(std_mpsc::Sender<Box<dyn FnOnce(JNIEnv) + Send>>);

impl JavaExecutor {
    pub fn new(java_vm: JavaVM) -> Self {
        let (tx, rx): (_, std_mpsc::Receiver<Box<dyn FnOnce(JNIEnv) + Send>>) =
            std_mpsc::channel();
        std::thread::Builder::new()
            .name(String::from("java-worker"))
            .spawn(move || {
                // Detach is performed automatically when thread exits.
                // Subsequent attach calls are no-op.
                let env = java_vm.attach();

                while let Ok(task) = rx.recv() {
                    task(env);
                    if env.exception_check() {
                        log::error!("java threw exception");
                        env.exception_describe();
                        env.exception_clear();
                    }
                }
            })
            .unwrap();

        Self(tx)
    }

    pub fn execute<T>(&self, task: T)
    where
        T: FnOnce(JNIEnv) + Send + 'static,
    {
        self.0.send(Box::new(task)).unwrap();
    }
}
