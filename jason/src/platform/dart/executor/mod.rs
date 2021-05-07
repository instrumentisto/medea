//! Implementation of the executor of [`Future`]s for the Dart environment.

mod task;

use std::{future::Future, mem, ptr::NonNull, rc::Rc};

use dart_sys::{Dart_CObject, Dart_CObjectValue, Dart_CObject_Type, Dart_Port};

use crate::platform::dart::utils::dart_api::Dart_PostCObject_DL_Trampolined;

use self::task::Task;

/// Runs a Rust Future on the current thread.
pub fn spawn(future: impl Future<Output = ()> + 'static) {
    let task = Task::new(Box::pin(future));

    // Task is leaked and will be freed by Dart calling the
    // rust_executor_drop_task().
    task_wake(NonNull::from(mem::ManuallyDrop::new(task).as_ref()));
}

/// A [`Dart_Port`] used to send [`Task`]'s poll commands so Dart will poll
/// Rust's futures.
///
/// Must be initialized with [`rust_executor_init`] during FFI initialization.
static mut WAKE_PORT: Option<Dart_Port> = None;

/// Initialize dart-driven async task executor.
///
/// On a Dart side you should continuously read channel to get [`Task`]s that
/// should be polled addresses
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn rust_executor_init(wake_port: Dart_Port) {
    WAKE_PORT = Some(wake_port);
}

/// Polls incomplete task.
///
/// This function returns `true` if task is still pending, and `false` if task
/// has finished. In this case it should be dropped with
/// [`rust_executor_drop_task`].
///
/// # Safety
///
/// Valid [`Task`] pointer must be provided. Must not be called if the
/// provided [`Task`] was dropped (with [`rust_executor_drop_task`]).
#[no_mangle]
pub unsafe extern "C" fn rust_executor_poll_task(
    mut task: NonNull<Task>,
) -> bool {
    task.as_mut().poll().is_pending()
}

/// Drops task.
///
/// Completed tasks should be dropped to avoid leaks.
///
/// In some unusual cases (say on emergency shutdown or when executed too long)
/// tasks may be deleted before completion.
///
/// # Safety
///
/// Valid [`Task`] pointer must be provided. Must be called only once for
/// specific [`Task`].
#[no_mangle]
pub unsafe extern "C" fn rust_executor_drop_task(task: NonNull<Task>) {
    drop(Rc::from_raw(task.as_ptr()))
}

/// Commands external executor to poll the provided [`Task`].
///
/// Sends command that contains the provided [`Task`] to the configured
/// [`WAKE_PORT`]. When received, Dart must poll it by calling
/// [`rust_executor_poll_task`].
fn task_wake(poll: NonNull<Task>) {
    let wake_port = unsafe { WAKE_PORT }.unwrap();

    let mut task_addr = Dart_CObject {
        type_: Dart_CObject_Type::Int64,
        value: Dart_CObjectValue {
            as_int64: poll.as_ptr() as i64,
        },
    };

    let enqueued =
        unsafe { Dart_PostCObject_DL_Trampolined(wake_port, &mut task_addr) };
    if !enqueued {
        log::warn!("Could not send message to Dart's native port");
        unsafe { rust_executor_drop_task(poll) };
    }
}
