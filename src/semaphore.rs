use core::{
    marker, mem,
    ops::{Deref, DerefMut},
    ptr::{self, NonNull},
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{Block, BlockState, Pointer};

#[derive(Debug)]
pub struct Semaphore<T> {
    inner: Pointer<T>,
    lock: AtomicBool,
}

impl<T> Semaphore<T> {
    pub const fn empty() -> Self {
        Self {
            inner: Pointer::empty(),
            lock: AtomicBool::new(false),
        }
    }

    pub fn lock(&self) -> Guard<'_, T> {
        while self.lock.compare_and_swap(false, true, Ordering::SeqCst) {
            sched_yield();
        }
        // The lock is held for the lifetime of this object
        // we store `false` on drop
        Guard { 0: self }
    }
}

#[derive(Debug)]
pub struct Guard<'a, T>(&'a Semaphore<T>);

impl<'a, T> Drop for Guard<'a, T> {
    fn drop(&mut self) {
        self.0.lock.store(false, Ordering::SeqCst);
    }
}

impl<'a, T> Deref for Guard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.inner.get() }
    }
}

impl<'a, T> DerefMut for Guard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // # Safety
        // We are guaranteed to only every have one alias to T
        // mut or otherwise
        unsafe { &mut *self.0.inner.get() }
    }
}

fn sched_yield() {
    unsafe {
        let _time = crate::syscall!(SCHED_YIELD);
    }
}
