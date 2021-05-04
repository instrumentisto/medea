//! Converter of Dart `Future` to the Rust [`Future`].

use std::future::Future;

use dart_sys::Dart_Handle;
use futures::channel::oneshot;

/// Converts provided [`Dart_Handle`] to the Rust [`Future`].
pub fn dart_future_to_rust(
    fut: Dart_Handle,
) -> impl Future<Output = Result<Dart_Handle, DartError>> {
    DartFuture::new(fut)
}

/// Error which can be obtained from the Dart side.
// TODO: temporary enum which will be replaced by real platform error.
pub struct DartError {}

impl From<Dart_Handle> for DartError {
    fn from(from: Dart_Handle) -> Self {
        Self {}
    }
}

/// Resolver of the Dart `Future`.
///
/// __Only needed for Dart side. Should not be used by Rust directly.__
pub struct DartFuture(oneshot::Sender<Result<Dart_Handle, DartError>>);

impl DartFuture {
    /// Spawns provided [`Dart_Handle`] on the Dart runtime and returns
    /// [`Future`] which will be resolved when Dart `Future` will be resolved.
    fn new(
        dart_fut: Dart_Handle,
    ) -> impl Future<Output = Result<Dart_Handle, DartError>> {
        let (tx, rx) = oneshot::channel();
        let this = Self(tx);

        unsafe {
            FUTURE_SPAWNER_CALLER.unwrap()(
                dart_fut,
                Box::into_raw(Box::new(this)),
            )
        };

        async move { rx.await.unwrap() }
    }

    /// Successfully resolves this [`DartFuture`] with the provided
    /// [`Dart_Handle`].
    fn resolve_ok(self, val: Dart_Handle) {
        let _ = self.0.send(Ok(val));
    }

    /// Resolves this [`DartFuture`] with the provided [`Dart_Handle`] error.
    fn resolve_err(self, val: Dart_Handle) {
        let _ = self.0.send(Err(DartError::from(val)));
    }
}

/// Pointer to an extern function that spawns Dart Future and notifies Rust when
/// it will be resolved.
type FutureSpawnerCaller = extern "C" fn(Dart_Handle, *mut DartFuture);

/// Stores pointer to the [`FutureSpawnerFunction`] extern function.
///
/// Should be initialized by Dart during FFI initialization phase.
static mut FUTURE_SPAWNER_CALLER: Option<FutureSpawnerCaller> = None;

/// Registers the provided [`FutureSpawnerCaller`] as
/// [`FUTURE_SPAWNER_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_future_spawner_caller(
    f: FutureSpawnerCaller,
) {
    FUTURE_SPAWNER_CALLER = Some(f);
}

/// Successfully resolves provided [`DartFuture`] with the provided
/// [`Dart_Handle`].
#[no_mangle]
pub unsafe extern "C" fn DartFuture__resolve_ok(
    fut: *mut DartFuture,
    val: Dart_Handle,
) {
    let fut = Box::from_raw(fut);
    fut.resolve_ok(val);
}

/// Resolves provided [`DartFuture`] with the provided [`Dart_Handle`] error.
#[no_mangle]
pub unsafe extern "C" fn DartFuture__resolve_err(
    fut: *mut DartFuture,
    val: Dart_Handle,
) {
    let fut = Box::from_raw(fut);
    fut.resolve_err(val);
}
