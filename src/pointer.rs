use std::{
    fmt, marker, mem,
    ptr::{self, NonNull},
};

use crate::{Block, BlockState};

#[derive(PartialEq, Eq, Clone)]
pub struct Pointer<T> {
    inner: NonNull<T>,
    /// We __own__ this `T`.
    _mk: marker::PhantomData<T>,
}

impl<T> fmt::Debug for Pointer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Ptr").field(&self.inner.as_ptr()).finish()
    }
}

impl<T> Pointer<T> {
    pub fn new(ptr: *mut T) -> Self {
        assert!(!ptr.is_null(), "null");
        // TODO can we check for null and return Pointer::empty if null?
        Self {
            inner: unsafe { NonNull::new_unchecked(ptr) },
            _mk: marker::PhantomData,
        }
    }

    pub const fn empty() -> Self {
        Self {
            inner: unsafe { NonNull::new_unchecked(0x1 as *mut T) },
            _mk: marker::PhantomData,
        }
    }

    /// Since we know we will only ever check for a null `Pointer<Block>` null being
    /// represented by 0x1 seems safe.
    pub fn is_null(&self) -> bool {
        self.inner.as_ptr() == 0x1 as *mut _
    }

    /// Cast our `Pointer<T>` to a `Pointer<U>`.
    pub fn cast<U>(&self) -> Pointer<U> {
        Pointer::new(self.inner.as_ptr().cast())
    }

    /// This will add some value multiplied by the size of T to the pointer.
    ///
    /// # Safety
    ///
    /// This is unsafe, due to OOB offsets being undefined behavior.
    pub unsafe fn offset(&self, offset: isize) -> Self {
        Pointer::new(self.inner.as_ptr().offset(offset))
    }

    /// This will add some value multiplied by the size of T to the pointer.
    ///
    /// # Safety
    ///
    /// This is unsafe, due to OOB offsets being undefined behavior.
    pub unsafe fn add(&self, offset: usize) -> Self {
        Pointer::new(self.inner.as_ptr().add(offset))
    }

    /// Return the underlying *mut pointer.
    pub fn get(&self) -> *mut T {
        self.inner.as_ptr()
    }
}

impl Pointer<Block> {
    pub fn next(&self) -> Pointer<Block> {
        debug_assert!(!self.is_null());
        unsafe { (*self.get()).next.clone() }
    }

    pub fn prev(&self) -> Pointer<Block> {
        debug_assert!(!self.is_null());
        unsafe { (*self.get()).prev.clone() }
    }

    pub fn is_free(&self) -> bool {
        debug_assert!(!self.is_null());
        unsafe { (*self.get()).free == BlockState::Free }
    }

    pub fn size(&self) -> usize {
        debug_assert!(!self.is_null());
        unsafe { (*self.get()).size }
    }

    pub fn mark_free(&self) {
        debug_assert!(!self.is_null());
        unsafe { (*self.get()).free = BlockState::Free };
    }

    pub fn mark_used(&self) {
        debug_assert!(!self.is_null());
        unsafe { (*self.get()).free = BlockState::InUse };
    }

    pub fn set_next(&self, next: Pointer<Block>) {
        debug_assert!(!self.is_null());
        unsafe { (*self.get()).next = next };
    }

    pub fn set_prev(&self, prev: Pointer<Block>) {
        debug_assert!(!self.is_null());
        unsafe { (*self.get()).prev = prev };
    }

    pub fn set_size(&self, size: usize) {
        debug_assert!(!self.is_null());
        unsafe { (*self.get()).size = size };
    }

    pub fn add_size(&self, size: usize) {
        debug_assert!(!self.is_null());
        unsafe {
            (*self.get()).size += size;
        };
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn pointer_null() {
        struct Thing {
            a: usize,
            b: String,
        }
        let ptr = Pointer::<Thing>::empty();

        assert!(ptr.is_null());
    }
}
