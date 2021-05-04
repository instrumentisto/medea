//! FFI part of [`Future`] executor.
//!
//! [`Future`]: std::future::Future

use super::{global, Box, BoxedPoll, UserData};

/// Raw external task handle
pub type ExternTask = UserData;

/// Raw external data
pub type ExternData = UserData;

/// Rust internal task handle
pub type InternTask = UserData;

/// Raw poll function
///
/// This function must be called to poll future on each wake event
pub type TaskPoll = extern "C" fn(InternTask) -> bool;

/// Raw C drop function
///
/// This function must be called to cleanup either pending or completed future
pub type TaskDrop = extern "C" fn(InternTask);

/// C function which can create new tasks
pub type TaskNew = extern "C" fn(ExternData) -> ExternTask;

/// C function which can run created tasks
pub type TaskRun = extern "C" fn(ExternTask, InternTask);

/// C function which can wake created task
///
/// This function will be called when pending future need to be polled again
pub type TaskWake = extern "C" fn(ExternTask);

/// Initialize async executor by providing task API calls
#[export_name = "rust_async_executor_init"]
pub extern "C" fn loop_init(
    task_new: TaskNew,
    task_run: TaskRun,
    task_wake: TaskWake,
    task_data: ExternData,
) {
    use global::*;

    unsafe {
        TASK_NEW = task_new as _;
        TASK_RUN = task_run as _;
        TASK_WAKE = task_wake as _;
        TASK_DATA = task_data as _;
    }
}

/// Task poll function which should be called to resume task
#[export_name = "rust_async_executor_poll"]
pub extern "C" fn task_poll(data: InternTask) -> bool {
    let poll = unsafe { &mut *(data as *mut BoxedPoll) };
    poll()
}

/// Task drop function which should be called to delete task
#[export_name = "rust_async_executor_drop"]
pub extern "C" fn task_drop(data: InternTask) {
    let _poll = unsafe { Box::from_raw(data as *mut BoxedPoll) };
}
