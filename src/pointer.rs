use core::{
    marker, mem,
    ptr::{self, NonNull},
};

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Pointer<T> {
    inner: NonNull<T>,
    /// We __own__ this `T`.
    _mk: marker::PhantomData<T>,
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

    /// Return the underlying *mut pointer.
    pub fn get(&self) -> *mut T {
        self.inner.as_ptr()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn pointer_null() {
        struct Thing {
            a: usize,
            b: usize,
        }
        let ptr = Pointer::<Thing>::empty();

        assert!(ptr.is_null());
    }
}
