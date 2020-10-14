use core::{
    fmt, mem,
    ops::{Deref, DerefMut},
    ptr::{self, NonNull},
    slice,
};

use crate::{
    mmap::mmap,
    sbrk,
    util::{align, extra_brk, MIN_ALIGN},
};

pub const BLOCK_ALIGN: usize = align(mem::size_of::<Block>()) as usize;

pub enum EmptyResult {
    Ok,
    Err,
}

impl EmptyResult {
    #[inline]
    pub fn is_err(&self) -> bool {
        matches!(self, Self::Err)
    }

    #[inline]
    pub fn unwrap(self) {
        if let Self::Err = self {
            panic!("called `unwrap` on an error value")
        }
    }
}

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
    fn is_null(&self) -> bool {
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

        unsafe {
            let item = ptr::read(self.ptr.as_ptr().add(idx));

            for idx in idx..self.len {
                ptr::write(
                    self.ptr.as_ptr().add(idx),
                    ptr::read(self.ptr.as_ptr().add(idx + 1)),
                );
            }
            self.len -= 1;
            item
        }
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
        write!(f, "{:?}", &self)
    }
}

impl<T: PartialEq> PartialEq<[T]> for RawSlice<T> {
    fn eq(&self, other: &[T]) -> bool {
        other == self.deref()
    }
}

impl RawSlice<Block> {
    pub fn find(&mut self, size: usize) -> Option<Block> {
        todo!()
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

pub struct Block {
    size: usize,
    data: NonNull<u8>,
    free: bool,
}

impl Block {
    /// Create a new `Block`, the block assumes it is in use.
    ///
    /// If for some reason you need a new free block `free` must be set to true.
    pub fn new(data: *mut u8, size: usize) -> Self {
        Self {
            size,
            data: unsafe { NonNull::new_unchecked(data) },
            free: false,
        }
    }

    pub fn is_null(&self) -> bool {
        self.data.as_ptr() == 0x1 as *mut _
    }

    /// Returns the data that this block represents.
    ///
    /// Adds `mem::size_of::<Block>()` to the offset of Blocks pointer.
    pub fn raw_data(&self) -> *mut u8 {
        unsafe { self.data.as_ptr().add(mem::size_of::<Self>()) }
    }

    pub fn merge_right(&mut self, next: Block) -> EmptyResult {
        if !next.is_null() {
            unsafe {
                ptr::write_bytes(next.data.as_ptr(), 0, next.size);
            }
        }
        self.size += next.size;
        EmptyResult::Ok
    }
}

pub struct BookKeeper {
    free: RawSlice<Block>,
    used: RawSlice<Block>,
}

impl BookKeeper {
    pub const fn new() -> Self {
        unsafe {
            Self {
                free: RawSlice::empty(),
                used: RawSlice::empty(),
            }
        }
    }

    pub unsafe fn extend_heap(&mut self, size: usize) -> *mut u8 {
        // Returns pointer to the next free chunk
        let mut b = sbrk(0).map(|ptr| Block::new(ptr as *mut _, size)).unwrap();

        if sbrk(size as isize).is_ok() {
            let data = b.raw_data();
            self.used.push(b);
            data
        } else {
            panic!("NEXT PAGE IS OOM??")
        }
    }

    pub fn allocate(&mut self, size: usize) -> *mut u8 {
        let need_size = align(size) as usize;

        if self.free.is_null() || self.used.is_null() {
            unsafe {
                // Init the RawSlice's as this is the first time around
                self.free = RawSlice::new(mmap(0).unwrap() as *mut Block, 0, 4096 / BLOCK_ALIGN);
                self.used = RawSlice::new(mmap(0).unwrap() as *mut Block, 0, 4096 / BLOCK_ALIGN);

                self.extend_heap(need_size)
            }
        } else if let Some(blk) = self.free.find(need_size) {
            let data = blk.raw_data();
            self.used.push(blk);
            data
        } else {
            unsafe { self.extend_heap(need_size) }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use core::ops::Deref;

    #[test]
    fn test_raw_slice() {
        assert!(unsafe { RawSlice::<Block>::empty().is_null() });

        let mut buffer = [b'a'; 32];
        let mut vec = unsafe { RawSlice::new(&mut buffer[0] as *mut u8, 16, 32) };

        assert_eq!(&*vec, b"aaaaaaaaaaaaaaaa");
        vec.push(b'b').unwrap();
        assert_eq!(&*vec, b"aaaaaaaaaaaaaaaab");
        vec.push(b'c').unwrap();
        assert_eq!(&*vec, b"aaaaaaaaaaaaaaaabc");
        vec[0] = b'.';
        assert_eq!(&*vec, b".aaaaaaaaaaaaaaabc");

        for _ in 0..14 {
            vec.push(b'_').unwrap();
        }

        assert_eq!(vec.pop().unwrap(), b'_');
        vec.push(b'@').unwrap();

        // push to the "33rd" index, this is an error since we only have 32
        assert!(vec.push(b'!').is_err());

        assert_eq!(&*vec, b".aaaaaaaaaaaaaaabc_____________@");

        for _ in 0..32 {
            vec.pop().unwrap();
        }

        assert!(vec.pop().is_none());
        assert!(vec.pop().is_none());
        assert!(vec.pop().is_none());
        assert!(vec.pop().is_none());
    }

    #[test]
    fn test_resize() {
        let mut buffer = [b'a'; 16];
        let mut vec = unsafe { RawSlice::new(&mut buffer[0] as *mut u8, 16, 16) };

        let mut new_buf = [b'b'; 20];
        vec.resize(&mut new_buf[0] as *mut u8, 20).unwrap();
        vec.push(b'b');
    }

    #[test]
    fn test_remove() {
        let mut buffer = [1u8, 2, 3, 4];
        let mut vec = unsafe { RawSlice::new(&mut buffer[0] as *mut u8, 4, 16) };

        let x = vec.remove(2);
        assert_eq!(x, 3);
        assert_eq!(&*vec, [1u8, 2, 4]);

        let mut buffer = [0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let mut vec = unsafe { RawSlice::new(&mut buffer[0] as *mut u8, 10, 16) };

        let x = vec.remove(3);
        assert_eq!(x, 3);
        assert_eq!(&*vec, [0u8, 1, 2, 4, 5, 6, 7, 8, 9]);

        let x = vec.remove(8);
        assert_eq!(x, 9);
        assert_eq!(&*vec, [0u8, 1, 2, 4, 5, 6, 7, 8]);

        let x = vec.remove(0);
        assert_eq!(x, 0);
        assert_eq!(&*vec, [1, 2, 4, 5, 6, 7, 8]);
    }

    #[test]
    #[rustfmt::skip]
    fn test_block_offset() {
        unsafe {
            let mut buf = [0u8; mem::size_of::<Block>() + mem::size_of::<u32>()];

            let blk = Block::new(&mut buf[0] as *mut _, mem::size_of::<u32>());
            let ptr = blk.raw_data();

            ptr::write(ptr as *mut u32, 10);

            assert_eq!(
                buf,
                [
                    0, 0, 0, 0,
                    0, 0, 0, 0,
                    0, 0, 0, 0,
                    0, 0, 0, 0,
                    0, 0, 0, 0,
                    0, 0, 0, 0,
                    10, 0, 0, 0
                ]
            );

            assert_eq!(*ptr, 10)
        }
    }
}
