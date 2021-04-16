use dart_sys::Dart_Handle;
use derive_more::From;

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

#[derive(From)]
pub struct DartList(Dart_Handle);

impl DartList {
    pub fn get(&self, i: i32) -> Option<Dart_Handle> {
        unsafe {
            // TODO: make it optional
            Some(GET_FUNCTION.unwrap()(self.0, i))
        }
    }

    pub fn length(&self) -> i32 {
        unsafe { LENGTH_FUNCTION.unwrap()(self.0) }
    }
}

impl<T> From<DartList> for Vec<T>
where
    T: From<Dart_Handle>,
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
