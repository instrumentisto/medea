use std::sync::{Condvar, Mutex};

use once_cell::sync::OnceCell;

static BROWSER_CLOSED: OnceCell<(Mutex<bool>, Condvar)> = OnceCell::new();

pub fn init() {
    BROWSER_CLOSED
        .set((Mutex::new(false), Condvar::new()))
        .unwrap();
}

pub fn browser_opened() {
    let (closed, _) = BROWSER_CLOSED.get().unwrap();
    *closed.lock().unwrap() = false;
}

pub fn browser_closed() {
    let (closed, cvar) = BROWSER_CLOSED.get().unwrap();
    *closed.lock().unwrap() = true;
    cvar.notify_one();
}

pub fn wait_for_browser_close() {
    let (closed, cvar) = BROWSER_CLOSED.get().unwrap();
    let mut closed = closed.lock().unwrap();
    while !*closed {
        closed = cvar.wait(closed).unwrap();
    }
}
