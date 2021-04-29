use std::ptr;

use libc::c_char;

use crate::utils::dart::into_dart_string;

#[repr(C)]
pub struct NullableChar {
    pub value: *const c_char,
    pub is_some: i8,
}

impl From<Option<String>> for NullableChar {
    fn from(from: Option<String>) -> Self {
        if let Some(from) = from {
            Self {
                value: unsafe { into_dart_string(from) },
                is_some: 1,
            }
        } else {
            Self {
                value: ptr::null(),
                is_some: 0,
            }
        }
    }
}

#[repr(C)]
pub struct NullableInt {
    pub value: i32,
    pub is_some: i8,
}

impl From<Option<i32>> for NullableInt {
    fn from(from: Option<i32>) -> Self {
        if let Some(from) = from {
            Self {
                value: from,
                is_some: 1,
            }
        } else {
            Self {
                value: 0,
                is_some: 0,
            }
        }
    }
}

impl From<Option<u16>> for NullableInt {
    fn from(from: Option<u16>) -> Self {
        from.map(|v| v as i32).into()
    }
}
