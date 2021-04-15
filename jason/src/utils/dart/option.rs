#[repr(C)]
pub struct DartOption<T> {
    some: T,
    is_none: bool,
}

impl<T> DartOption<T> {}
