use std::any::Any;

#[repr(C)]
pub struct DartOption {
    is_some: i8,
    val: *mut dyn Any,
}

impl<T: 'static> From<Option<T>> for DartOption {
    fn from(from: Option<T>) -> Self {
        if let Some(from) = from {
            Self {
                is_some: 1,
                val: Box::into_raw(Box::new(from)),
            }
        } else {
            Self {
                is_some: 0,
                val: Box::into_raw(Box::new(())),
            }
        }
    }
}

impl<'a, T: 'static> Into<Option<&'a T>> for DartOption {
    fn into(self) -> Option<&'a T> {
        if self.is_some == 1 {
            unsafe {
                Some(self.val.as_ref().unwrap().downcast_ref().unwrap())
            }
        } else {
            None
        }
    }
}