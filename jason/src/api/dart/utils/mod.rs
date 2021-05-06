mod arrays;
mod string;

pub use self::{
    arrays::PtrArray,
    string::{c_str_into_string, string_into_c_str},
};
