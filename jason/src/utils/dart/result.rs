use dart_sys::Dart_Handle;
use crate::platform::dart::error::Error;
use crate::utils::dart::from_dart_string;

pub struct DartResult(Dart_Handle);

type IsOkFunction = extern "C" fn(Dart_Handle) -> u8;
static mut IS_OK_FUNCTION: Option<IsOkFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_DartResult__is_ok(
    f: IsOkFunction,
) {
    IS_OK_FUNCTION = Some(f);
}

type OkDartHandleFunction = extern "C" fn(Dart_Handle) -> Dart_Handle;
static mut OK_DART_HANDLE_FUNCTION: Option<OkDartHandleFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_DartResult__ok_dart_handle(
    f: OkDartHandleFunction,
) {
    OK_DART_HANDLE_FUNCTION = Some(f);
}

#[repr(C)]
pub struct DartError {
    pub name: *const libc::c_char,
    pub message: *const libc::c_char,
}

impl From<DartError> for Error {
    fn from(err: DartError) -> Self {
        Self {
            name: unsafe { from_dart_string(err.name) },
            message: unsafe { from_dart_string(err.message) },
        }
    }
}

type ErrFunction = extern "C" fn(Dart_Handle) -> DartError;
static mut ERR_FUNCTION: Option<ErrFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_DartResult__err(
    f: ErrFunction,
) {
    ERR_FUNCTION = Some(f);
}

impl DartResult {
    pub fn is_ok(&self) -> bool {
        unsafe { IS_OK_FUNCTION.unwrap()(self.0) == 1 }
    }

    pub fn ok_dart_handle(&self) -> Dart_Handle {
        unsafe { OK_DART_HANDLE_FUNCTION.unwrap()(self.0) }
    }

    pub fn err(&self) -> DartError {
        unsafe {
            ERR_FUNCTION.unwrap()(self.0)
        }
    }
}

impl From<DartResult> for Result<Dart_Handle, Error> {
    fn from(res: DartResult) -> Self {
        if res.is_ok() {
            Ok(res.ok_dart_handle())
        } else {
            Err(res.err().into())
        }
    }
}

