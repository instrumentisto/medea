use dart_sys::Dart_Handle;
use futures::channel::oneshot;
use futures::channel::oneshot::Canceled;
use std::future::Future;

pub struct DartFuture(oneshot::Sender<Dart_Handle>);

impl DartFuture {
    pub fn new(dart_fut: Dart_Handle) -> impl Future<Output = Result<Dart_Handle, Canceled>> {
        let (tx, rx) = oneshot::channel();
        let this = Self(tx);

        unsafe { FUTURE_SPAWNER_FUNCTION.unwrap()(dart_fut, Box::into_raw(Box::new(this))) };

        rx
    }

    fn resolve(self, val: Dart_Handle) {
        self.0.send(val);
    }
}

type FutureSpawnerFunction = extern "C" fn(Dart_Handle, *mut DartFuture);
static mut FUTURE_SPAWNER_FUNCTION: Option<FutureSpawnerFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_spawn_dart_future_function(f: FutureSpawnerFunction) {
    FUTURE_SPAWNER_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn dart_future_resolved(
    fut: *mut DartFuture,
    val: Dart_Handle,
) {
    let fut = Box::from_raw(fut);
    fut.resolve(val);
}
