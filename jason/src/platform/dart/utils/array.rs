use dart_sys::Dart_Handle;
use derive_more::From;

type GetFunction = extern "C" fn(Dart_Handle, i32) -> Dart_Handle;
static mut GET_FUNCTION: Option<GetFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Array__get_function(
    f: GetFunction,
) {
    GET_FUNCTION = Some(f);
}

pub struct Array(Dart_Handle);

impl Array {
    pub fn get(&self, i: i32) -> Option<Dart_Handle> {
        unsafe {
            // TODO: make it optional
            Some(GET_FUNCTION.unwrap()(self.0, i))
        }
    }
}