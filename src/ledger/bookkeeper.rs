use core::{
    cmp, fmt, mem,
    ops::{Deref, DerefMut},
    ptr::{self, NonNull},
    slice,
};

use crate::{
    ledger::{block::Block, raw_slice::RawSlice, BLOCK_ALIGN},
    mmap::mmap,
    sbrk,
    util::{align, extra_brk, MIN_ALIGN},
};

/// The `BookKeeper` acts as our arena, it keeps `Chunks` of different size classes
/// sorted in ascending order smallest -> largest.
///
/// We use a `free` and `used` tree to represent currently used and
/// currently free but still owned by the arena (not returned to the os).
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

    /// `size` needs to be aligned or pre whatevered before this is called.
    ///
    /// # Safety
    ///
    /// This will panic on OOM or if sbrk fails in anyother way.
    pub unsafe fn extend_heap(&mut self, size: usize) -> *mut u8 {
        // Returns pointer to the next free chunk
        let mut b = sbrk(0).map(|ptr| Block::new(ptr as *mut _, size)).unwrap();

        if sbrk(size as isize).is_ok() {
            let data = b.raw_data();
            // Pretty sure we should __NEVER__ find the same pointer when allocating from
            // sbrk or mmap...
            let idx = self.used.binary_search(&b).unwrap_err();
            self.used.insert(idx, b);
            data
        } else {
            panic!("NEXT PAGE IS OOM??")
        }
    }

    /// `size` needs to be the raw __unalinged__ size of the reqeusted memory.
    pub fn allocate(&mut self, size: usize) -> *mut u8 {
        // TODO save `size` but use `aligned_size`
        let aligned_size = align(size) as usize;

        // This is a lot of allocation so make sure it only happens once
        if self.free.is_null() && self.used.is_null() {
            unsafe {
                // Init the RawSlice's as this is the first time around
                self.free = RawSlice::new(
                    mmap(crate::mmap::PAGE_SIZE as isize).unwrap() as *mut Block,
                    0,
                    crate::mmap::PAGE_SIZE / BLOCK_ALIGN,
                );
                self.used = RawSlice::new(
                    mmap(crate::mmap::PAGE_SIZE as isize).unwrap() as *mut Block,
                    0,
                    crate::mmap::PAGE_SIZE / BLOCK_ALIGN,
                );

                self.extend_heap(aligned_size)
            }
        // `find(...)` handles splitting and merging of blocks
        } else if let Some(blk) = self.free.find(aligned_size) {
            // "copy" the pointer before pushing the blk onto the used list
            let data = blk.raw_data();
            self.used.push(blk);
            data
        } else {
            unsafe { self.extend_heap(aligned_size) }
        }
    }

    pub fn free(&mut self, ptr: *mut u8) {
        let idx = self
            .used
            .binary_search(&Block::new(ptr, 0))
            .unwrap_or_else(|x| x);
    }
}

#[cfg(test)]
mod test {
    use super::*;

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
    fn test_insert() {
        let mut buffer = [1u8, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut vec = unsafe { RawSlice::new(&mut buffer[0] as *mut u8, 4, 16) };

        vec.insert(2, 10);
        assert_eq!(&*vec, [1u8, 2, 10, 3, 4]);
        assert_eq!(vec.len(), 5);

        let mut buffer = [0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 0, 0, 0, 0, 0];
        let mut vec = unsafe { RawSlice::new(&mut buffer[0] as *mut u8, 10, 16) };

        vec.insert(0, 10);
        assert_eq!(&*vec, [10u8, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(vec.len(), 11);

        vec.insert(11, 10);
        assert_eq!(&*vec, [10u8, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert_eq!(vec.len(), 12);
    }

    #[test]
    fn test_alloc() {
        let mut bk = BookKeeper::new();

        let ptr = bk.allocate(mem::size_of::<u32>());
        unsafe {
            ptr::write(ptr as *mut u32, 11u32);
            assert_eq!(*(ptr as *mut u32), 11u32)
        }
    }
}
