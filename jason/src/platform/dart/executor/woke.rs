use std::{
    marker::PhantomData,
    mem,
    mem::ManuallyDrop,
    ops::Deref,
    rc::Rc,
    task::{RawWaker, RawWakerVTable, Waker},
};

pub trait Woke {
    fn wake(self: Rc<Self>) {
        Self::wake_by_ref(&self)
    }

    fn wake_by_ref(arc_self: &Rc<Self>);
}

pub fn waker_vtable<W: Woke>() -> &'static RawWakerVTable {
    &RawWakerVTable::new(
        clone_arc_raw::<W>,
        wake_arc_raw::<W>,
        wake_by_ref_arc_raw::<W>,
        drop_arc_raw::<W>,
    )
}

unsafe fn increase_refcount<T: Woke>(data: *const ()) {
    let arc = mem::ManuallyDrop::new(Rc::<T>::from_raw(data.cast::<T>()));
    let _arc_clone: mem::ManuallyDrop<_> = arc.clone();
}

unsafe fn clone_arc_raw<T: Woke>(data: *const ()) -> RawWaker {
    increase_refcount::<T>(data);
    RawWaker::new(data, waker_vtable::<T>())
}

unsafe fn wake_arc_raw<T: Woke>(data: *const ()) {
    let arc: Rc<T> = Rc::from_raw(data.cast::<T>());
    Woke::wake(arc);
}

unsafe fn wake_by_ref_arc_raw<T: Woke>(data: *const ()) {
    // Retain Arc, but don't touch refcount by wrapping in ManuallyDrop
    let arc = mem::ManuallyDrop::new(Rc::<T>::from_raw(data.cast::<T>()));
    Woke::wake_by_ref(&arc);
}

unsafe fn drop_arc_raw<T: Woke>(data: *const ()) {
    drop(Rc::<T>::from_raw(data.cast::<T>()))
}

#[derive(Debug)]
pub struct WakerRef<'a> {
    waker: ManuallyDrop<Waker>,
    _marker: PhantomData<&'a ()>,
}

impl<'a> WakerRef<'a> {
    pub fn new_unowned(waker: ManuallyDrop<Waker>) -> Self {
        WakerRef {
            waker,
            _marker: PhantomData,
        }
    }
}

impl Deref for WakerRef<'_> {
    type Target = Waker;

    fn deref(&self) -> &Waker {
        &self.waker
    }
}

#[inline]
pub fn waker_ref<W>(wake: &Rc<W>) -> WakerRef<'_>
where
    W: Woke,
{
    let ptr = (&**wake as *const W).cast::<()>();

    let waker = ManuallyDrop::new(unsafe {
        Waker::from_raw(RawWaker::new(ptr, waker_vtable::<W>()))
    });
    WakerRef::new_unowned(waker)
}
