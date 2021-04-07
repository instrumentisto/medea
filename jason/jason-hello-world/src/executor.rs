use extern_executor::{
    dart::{DartPort, DartPostCObject},
    ffi::ExternTask,
};

/// Reexport extern "C" functions from extern_executor.
///
/// Workaround for rust-lang/rust#6342.
#[macro_export]
macro_rules! export_c_symbol {
    (fn $name:ident($( $arg:ident : $type:ty ),*) -> $ret:ty) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name($( $arg : $type),*) -> $ret {
            ::extern_executor::dart::$name($( $arg ),*)
        }
    };
    (fn $name:ident($( $arg:ident : $type:ty ),*)) => {
        export_c_symbol!(fn $name($( $arg : $type),*) -> ());
    }
}

export_c_symbol!(fn loop_init(wake_port: DartPort, task_post: DartPostCObject));
export_c_symbol!(fn task_poll(task: ExternTask) -> bool);
export_c_symbol!(fn task_drop(task: ExternTask));
