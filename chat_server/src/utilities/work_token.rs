use std::sync::{Arc, Mutex, Condvar};
use std::sync::atomic::{AtomicBool, Ordering};
use std::any::Any;
use core::borrow::Borrow;

//TODO:
// Consider allowing wrapping type inside the token, making it easier to pass it around
// Consider Rustify this code a little (currently for simplicity, it is a dumb copy of a C++ code I have)
#[derive(Clone)]
pub struct Token(Arc<WorkToken>);

// TODO: Split into two similar types cancellation_token and work_token which will do the same, but with different method naming
impl Token {
    pub fn build() -> Token {
       Token(Arc::new(WorkToken::build()))
    }

    pub fn ready(&self) -> bool {
        self.0.ready()
    }

    pub fn canceled(&self) -> bool {
        self.0.ready()
    }

    pub fn done(&self) {
        self.0.done()
    }

    pub fn cancel(&self) {
        self.0.done()
    }

    pub fn wait(&self) {
        self.0.wait()
    }
}
// Mutex is used to sync the changes to the ready state with the condition variable
// It is required, as otherwise the thread checking the condition variable might miss notification
//      coming after checking the ready member, but before going to wait again on the condition variable
// We use atomic boolean for the state so we could check the state without taking a lock.
//      Of course it means that the state might not be fully up to date.
// I'm pretty sure that for boolean it is not actually required (on x86_64 it is not required for sure)
//      but you won't get that guarantee for cross platform code.
// Although all of the above, it still might be better to use a regular boolean while using locks to read the state.
//      of course it is usage dependent.
pub struct WorkToken {
    ready : AtomicBool,
    mutex : Mutex<()>,
    cv : Condvar
}

impl WorkToken {
    pub fn build() -> WorkToken {
        WorkToken {
            ready : AtomicBool::new(false),
            mutex : Mutex::new(()),
            cv : Condvar::new()
        }
    }
    // We don't promise this will return the latest value, as we are not using a lock.
    // We just guarantee the value returned will be valid(e.g. no changes mid=flight).
    pub fn ready(&self) -> bool {
        return self.ready.load(Ordering::Relaxed);
    }

    pub fn done(&self) {
        // We unwrap as threads should not panic while holding the lock
        let guard = self.mutex.lock().unwrap();
        self.ready.store(true, Ordering::Relaxed);
        std::mem::drop(guard);
        self.cv.notify_all();
    }

    pub fn wait(&self) {
        // We unwrap the condition variable and the lock as threads holding the lock shouldn't be panicking.
        let _guard = self.cv.wait_until(self.mutex.lock().unwrap(), |_| {
            self.ready.load(Ordering::Relaxed)
        }).unwrap();
    }
}