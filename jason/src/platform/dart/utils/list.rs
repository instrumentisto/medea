use dart_sys::Dart_Handle;

use crate::platform::dart::utils::handle::DartHandle;

type GetFunction = extern "C" fn(Dart_Handle, i32) -> Dart_Handle;
static mut GET_FUNCTION: Option<GetFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Array__get_function(f: GetFunction) {
    GET_FUNCTION = Some(f);
}

type LengthFunction = extern "C" fn(Dart_Handle) -> i32;
static mut LENGTH_FUNCTION: Option<LengthFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Array__length(f: LengthFunction) {
    LENGTH_FUNCTION = Some(f);
}

pub struct DartList(DartHandle);

impl From<Dart_Handle> for DartList {
    fn from(handle: Dart_Handle) -> Self {
        Self(handle.into())
    }
}

impl DartList {
    pub fn get(&self, i: i32) -> Option<DartHandle> {
        unsafe {
            // TODO: make it optional
            Some(GET_FUNCTION.unwrap()(self.0.get(), i).into())
        }
    }

    pub fn length(&self) -> i32 {
        unsafe { LENGTH_FUNCTION.unwrap()(self.0.get()) }
    }
}

impl<T> From<DartList> for Vec<T>
where
    T: From<DartHandle>,
{
    fn from(list: DartList) -> Self {
        let len = list.length();
        let mut out = Vec::with_capacity(len as usize);
        for i in 0..len {
            let val = list.get(i).unwrap();
            out.push(val.into())
        }
        out
    }
}
