use core::{
    cmp, fmt, mem,
    ops::{Deref, DerefMut},
    ptr::{self, NonNull},
    slice,
};

use crate::{
    ledger::{block::Block, EmptyResult, SPLIT_FACTOR},
    mmap::mmap,
    sbrk,
    util::{align, extra_brk, MIN_ALIGN},
};

pub struct RawSlice<T> {
    len: usize,
    cap: usize,
    ptr: NonNull<T>,
}

impl<T> RawSlice<T> {
    pub const unsafe fn empty() -> Self {
        Self {
            len: 0,
            cap: 0,
            // This seems valid as long as T is not a number?
            ptr: NonNull::new_unchecked(0x1 as *mut T),
        }
    }

    /// This ensures that we never attempt to `Deref` a null pointer.
    pub fn is_null(&self) -> bool {
        self.ptr.as_ptr() == 0x1 as *mut _
    }

    /// Create a new `RawSlice` from the given chunk of memory.
    ///
    /// # Safety
    /// Unsafe since we rely on `ptr` being large enough to hold `cap`.
    pub unsafe fn new(ptr: *mut T, len: usize, cap: usize) -> Self {
        Self {
            len,
            cap,
            ptr: unsafe { NonNull::new_unchecked(ptr) },
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }

    pub fn push(&mut self, item: T) -> EmptyResult {
        if self.len == self.cap {
            EmptyResult::Err
        } else {
            // If we have the cap this should never fail
            unsafe { ptr::write(self.ptr.as_ptr().add(self.len), item) };

            self.len += 1;

            EmptyResult::Ok
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            // Can't hit < 0 because of the empty check
            self.len -= 1;
            Some(unsafe { ptr::read(self.ptr.as_ptr().add(self.len)) })
        }
    }

    pub fn remove(&mut self, idx: usize) -> T {
        assert!(idx < self.len);

        let len = self.len();

        unsafe {
            let ptr = self.ptr.as_ptr().add(idx);

            // Copy the element, this is unsafe since
            // for two lines we have the element aliased twice
            let item = ptr::read(ptr);

            ptr::copy(ptr.offset(1), ptr, len - idx - 1);

            self.len -= 1;

            item
        }
    }

    pub fn insert(&mut self, idx: usize, item: T) {
        assert!(
            idx <= self.len && self.cap > self.len + 1,
            "len: {} cap: {} idx: {}",
            self.len,
            self.cap,
            idx
        );

        let len = self.len();

        unsafe {
            // Add to the len, shifting everything over by one
            let ptr = self.ptr.as_ptr().add(idx);

            // Shift everything over, this leaves element at idx
            // doubled
            // `[1, 2, 3, 3, 4]`
            //         ^ insert whatever at index 2 would result in that
            ptr::copy(ptr, ptr.offset(1), len - idx);

            // finally write the new element, overwriting the copied `idx`th value
            ptr::write(ptr, item);
        }
        self.len += 1;
    }

    pub fn resize(&mut self, new_ptr: *mut T, new_cap: usize) -> EmptyResult {
        if self.len <= new_cap {
            unsafe {
                let mut old = mem::replace(self, RawSlice::empty());
                ptr::copy_nonoverlapping(old.as_ptr(), new_ptr, old.len);

                self.cap = new_cap;
                self.len = old.len;
                self.ptr = NonNull::new_unchecked(new_ptr);
            }
            EmptyResult::Ok
        } else {
            EmptyResult::Err
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for RawSlice<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.deref())
    }
}

impl<T: PartialEq> PartialEq<[T]> for RawSlice<T> {
    fn eq(&self, other: &[T]) -> bool {
        other == self.deref()
    }
}

impl RawSlice<Block> {
    /// Pop `Block`s off the end until we have found a `Block`
    /// of suitable `size` to fit the request.
    ///
    /// This should only be called for the `free` `RawSlice` of `BookKeeper`.
    pub fn find(&mut self, size: usize) -> Option<Block> {
        let mut blk: Option<Block> = None;
        while let Some(mut next) = self.pop() {
            if let Some(curr) = &mut blk {
                // Absorb the next block and zero it out
                // adding `next.size` to `curr.size` if curr.size is to small
                if curr.size >= size {
                    // TODO: I think we want to split first to keep the split blocks
                    // next to each other? This also probably does not matter so who knows?
                    // Zeroing out the split block just incase may be helpful again who knows?

                    // If the block is "too large" split it into 2 equal
                    // sized blocks
                    if curr.size > size * SPLIT_FACTOR {
                        self.push(curr.split(size));
                    }

                    // Put the `next` Block back since we won't use it
                    // This check is probably redundant TODO
                    if Some(&curr) != Some(&&mut next) {
                        self.push(next).unwrap();
                    }

                    break;
                } else {
                    curr.merge_right(next);
                }
            } else {
                blk = Some(next);
            }
        }

        blk
    }
}

impl<T> Deref for RawSlice<T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        if self.is_null() {
            panic!("deref a null pointer")
        }

        unsafe {
            // The invariants maintains safety.
            slice::from_raw_parts(self.ptr.as_ptr() as *const T, self.len)
        }
    }
}

impl<T> DerefMut for RawSlice<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        if self.is_null() {
            panic!("deref a null pointer")
        }

        unsafe {
            // The invariants maintains safety.
            slice::from_raw_parts_mut(self.ptr.as_ptr() as *mut T, self.len)
        }
    }
}
