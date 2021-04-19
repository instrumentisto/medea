mod arrays;
mod string;

pub use self::{
    arrays::PtrArray,
    string::{c_str_into_string, string_into_c_str},
};

pub unsafe fn ptr_from_dart_as_mut<'a, T>(ptr: *mut T) -> &'a mut T {
    match ptr.as_mut() {
        Some(reference) => reference,
        None => {
            unimplemented!()
        }
    }
}

pub unsafe fn ptr_from_dart_as_ref<'a, T>(ptr: *const T) -> &'a T {
    match ptr.as_ref() {
        Some(reference) => reference,
        None => {
            unimplemented!()
        }
    }
}
