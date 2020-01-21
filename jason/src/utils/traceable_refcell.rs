use std::cell::{RefCell, RefMut, Ref};

macro_rules! invocation_place {
    () => {
        format!("{}:{}", file!(), line!())
    }
}

macro_rules! borrow {
    ($cell:expr) => {
        $cell.borrow(invocation_place!())
    }
}

macro_rules! borrow_mut {
    ($cell:expr) => {
        $cell.borrow_mut(invocation_place!())
    }
}

pub struct TraceableRefCell<T> {
    cell: RefCell<T>,
    current_borrow_mut_place: RefCell<Option<String>>,
    last_borrow_place: RefCell<Option<String>>,
}

impl<T> TraceableRefCell<T> {
    pub fn new(data: T) -> Self {
        Self {
            cell: RefCell::new(data),
            current_borrow_mut_place: RefCell::new(None),
            last_borrow_place: RefCell::new(None),
        }
    }

    pub fn borrow_mut(&self, invocation_place: String) -> RefMut<T> {
        if let Ok(ref_mut) = self.cell.try_borrow_mut() {
            web_sys::console::debug_1(&format!("Borrow mut in {}", invocation_place).into());
            *self.current_borrow_mut_place.borrow_mut() = Some(invocation_place);
            return ref_mut
        } else {
            panic!("RefCell BorrowMutError. Borrow mut place: {:?}. Last borrow place: {:?}. Where called: {}.", self.current_borrow_mut_place.borrow(), self.last_borrow_place.borrow(), invocation_place);
        }
    }

    pub fn borrow(&self, invocation_place: String) -> Ref<T> {
        if let Ok(ref_mut) = self.cell.try_borrow() {
            web_sys::console::debug_1(&format!("Borrow in {}", invocation_place).into());
            *self.last_borrow_place.borrow_mut() = Some(invocation_place);
            return ref_mut
        } else {
            panic!("RefCell BorrowError. Borrow mut place: {:?}. Last borrow place: {:?}. Where called: {}.", self.current_borrow_mut_place.borrow(), self.last_borrow_place.borrow(), invocation_place);
        }
    }
}
