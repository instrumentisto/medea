mod arrays;
mod callback;
mod string;
mod trampoline;

pub use self::{
    arrays::PtrArray,
    string::{c_str_into_string, string_into_c_str},
};
