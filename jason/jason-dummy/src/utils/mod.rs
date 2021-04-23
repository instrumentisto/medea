mod arrays;
mod closure;
mod completer;
mod dart_api;
mod string;

use std::future::Future;

pub use self::{
    arrays::PtrArray,
    closure::DartClosure,
    completer::Completer,
    string::{c_str_into_string, string_into_c_str},
};

pub fn spawn<F>(f: F)
where
    F: Future<Output = ()> + 'static,
{
    todo!()
}
