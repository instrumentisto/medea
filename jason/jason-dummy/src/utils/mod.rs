mod arrays;
mod callback;
mod completer;
mod string;
mod trampoline;

use std::future::Future;

pub use self::{
    arrays::PtrArray,
    callback::DartCallback,
    completer::Completer,
    string::{c_str_into_string, string_into_c_str},
};

pub fn spawn<F>(f: F)
where
    F: Future<Output = ()> + 'static,
{
    todo!()
}
