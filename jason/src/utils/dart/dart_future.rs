use crate::platform::dart::{error::DartError, utils::handle::DartHandle};
use dart_sys::Dart_Handle;
use futures::channel::oneshot;
use std::future::Future;

pub struct VoidDartFuture(oneshot::Sender<()>);

impl VoidDartFuture {
    pub fn new(dart_fut: Dart_Handle) -> impl Future<Output = ()> {
        let (tx, rx) = oneshot::channel();
        let this = Self(tx);

        unsafe {
            VOID_FUTURE_SPAWNER_FUNCTION.unwrap()(
                dart_fut,
                Box::into_raw(Box::new(this)),
            )
        };

        async move { rx.await.unwrap() }
    }

    pub fn resolve(self) {
        self.0.send(());
    }
}

pub struct DartFuture(oneshot::Sender<Result<DartHandle, DartError>>);

impl DartFuture {
    pub fn new(
        dart_fut: Dart_Handle,
    ) -> impl Future<Output = Result<DartHandle, DartError>> {
        let (tx, rx) = oneshot::channel();
        let this = Self(tx);

        unsafe {
            FUTURE_SPAWNER_FUNCTION.unwrap()(
                dart_fut,
                Box::into_raw(Box::new(this)),
            )
        };

        async move { rx.await.unwrap() }
    }

    fn resolve_ok(self, val: Dart_Handle) {
        let _ = self.0.send(Ok(DartHandle::new(val)));
    }

    fn resolve_err(self, val: Dart_Handle) {
        let _ = self.0.send(Err(DartError::from(val)));
    }
}

type VoidFutureSpawnerFunction =
    extern "C" fn(Dart_Handle, *mut VoidDartFuture);
static mut VOID_FUTURE_SPAWNER_FUNCTION: Option<VoidFutureSpawnerFunction> =
    None;

#[no_mangle]
pub unsafe extern "C" fn register_void_future_spawner_function(
    f: VoidFutureSpawnerFunction,
) {
    VOID_FUTURE_SPAWNER_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn VoidDartFuture__resolve(fut: *mut VoidDartFuture) {
    let fut = Box::from_raw(fut);
    fut.resolve();
}

type FutureSpawnerFunction = extern "C" fn(Dart_Handle, *mut DartFuture);
static mut FUTURE_SPAWNER_FUNCTION: Option<FutureSpawnerFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_spawn_dart_future_function(
    f: FutureSpawnerFunction,
) {
    FUTURE_SPAWNER_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn DartFuture__resolve_ok(
    fut: *mut DartFuture,
    val: Dart_Handle,
) {
    let fut = Box::from_raw(fut);
    fut.resolve_ok(val);
}

#[no_mangle]
pub unsafe extern "C" fn DartFuture__resolve_err(
    fut: *mut DartFuture,
    val: Dart_Handle,
) {
    let fut = Box::from_raw(fut);
    fut.resolve_err(val);
}
