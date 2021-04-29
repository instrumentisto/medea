pub mod dart;
pub mod ffi;
mod task;
mod types;
mod userdata;
mod woke;

mod global {
    use super::{null_mut, UserData};

    pub static mut TASK_NEW: UserData = null_mut();
    pub static mut TASK_RUN: UserData = null_mut();
    pub static mut TASK_WAKE: UserData = null_mut();
    pub static mut TASK_DATA: UserData = null_mut();
}

use ffi::*;
use task::*;
use types::*;
use userdata::*;

use self::{
    dart::{DartPort, DartPostCObject},
    ffi::ExternTask,
};

/// Spawn task
///
/// Create task for future and run it
pub fn spawn(future: impl Future + 'static) {
    let future = Box::pin(future);

    let task_new: TaskNew = unsafe { transmute(global::TASK_NEW) };
    let task_run: TaskRun = unsafe { transmute(global::TASK_RUN) };
    let task_data: ExternData = unsafe { global::TASK_DATA };

    let task = task_new(task_data);
    task_run(task, task_wrap(future, task));
}

/// Reexport extern "C" functions from extern_executor.
///
/// Workaround for rust-lang/rust#6342.
macro_rules! export_c_symbol {
    (fn $name:ident($( $arg:ident : $type:ty ),*) -> $ret:ty) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name($( $arg : $type),*) -> $ret {
            self::dart::$name($( $arg ),*)
        }
    };
    (fn $name:ident($( $arg:ident : $type:ty ),*)) => {
        export_c_symbol!(fn $name($( $arg : $type),*) -> ());
    }
}

export_c_symbol!(fn loop_init(wake_port: DartPort, task_post: DartPostCObject));
export_c_symbol!(fn task_poll(task: ExternTask) -> bool);
export_c_symbol!(fn task_drop(task: ExternTask));