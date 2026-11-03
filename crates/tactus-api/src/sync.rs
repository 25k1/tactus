use std::sync::{Mutex, MutexGuard, PoisonError};

pub trait Guard<T> {
    fn guard(&self) -> MutexGuard<'_, T>;
}

impl<T> Guard<T> for Mutex<T> {
    fn guard(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
