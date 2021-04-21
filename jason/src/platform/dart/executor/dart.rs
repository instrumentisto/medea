use dart_sys::{Dart_CObject, Dart_CObjectValue, Dart_CObject_Type, Dart_Port};

use super::{
    ffi,
    ffi::{ExternData, ExternTask, InternTask},
    null_mut, transmute, UserData,
};

#[allow(non_camel_case_types)]
type Dart_PostCObject_Fn = fn(Dart_Port, *mut Dart_CObject) -> bool;

/// Dart's port identifier
///
/// The port identifier for wake notifications should be set on initializing
/// event loop
pub type DartPort = i64;

/// Dart's data structure
pub struct DartCObject;

/// Dart's function which is used to send datas to ports
///
/// The pointer to this function should be set on initializing event loop
pub type DartPostCObject = fn(DartPort, *mut DartCObject) -> bool;

pub(crate) mod global {
    use super::*;

    pub static mut WAKE_PORT: Dart_Port = 0;
    pub static mut TASK_POST: UserData = null_mut();
}

#[repr(transparent)]
struct DartTask {
    data: InternTask,
}

extern "C" fn task_new(_data: ExternData) -> ExternTask {
    Box::into_raw(Box::new(DartTask { data: null_mut() })) as _
}

extern "C" fn task_run(task: ExternTask, data: InternTask) {
    {
        let mut task = unsafe { &mut *(task as *mut DartTask) };
        task.data = data;
    }
    task_wake(task);
}

/// Poll incomplete task
///
/// This function returns true when task is still pending and needs to be polled
/// yet. When task did completed false will be returned. In that case the task
/// is free to drop.
#[export_name = "rust_async_executor_dart_poll"]
pub extern "C" fn task_poll(task: ExternTask) -> bool {
    let task = unsafe { &mut *(task as *mut DartTask) };
    ffi::task_poll(task.data)
}

/// Delete task
///
/// Completed tasks should be dropped to avoid leaks.
///
/// In some unusual cases (say on emergency shutdown or when executed too long)
/// tasks may be deleted before completion.
#[export_name = "rust_async_executor_dart_drop"]
pub extern "C" fn task_drop(task: ExternTask) {
    let task = unsafe { Box::from_raw(task as *mut DartTask) };
    ffi::task_drop(task.data);
}

extern "C" fn task_wake(task: ExternTask) {
    use global::*;

    let wake_port = unsafe { WAKE_PORT };
    let task_post: Dart_PostCObject_Fn = unsafe { transmute(TASK_POST) };

    let mut task_addr = Dart_CObject {
        type_: Dart_CObject_Type::Int64,
        value: Dart_CObjectValue {
            as_int64: task as i64,
        },
    };

    task_post(wake_port, &mut task_addr);
}

/// Initialize dart-driven async task executor
///
/// On a Dart side you should continuously read channel to get task addresses
/// which needs to be polled.
#[export_name = "rust_async_executor_dart_init"]
pub extern "C" fn loop_init(wake_port: DartPort, task_post: DartPostCObject) {
    use global::*;

    unsafe {
        WAKE_PORT = wake_port;
        TASK_POST = task_post as UserData;
    }

    ffi::loop_init(task_new, task_run, task_wake, null_mut());
}
