use core::{
    cmp, fmt, mem,
    ops::{Deref, DerefMut},
    ptr::{self, NonNull},
    slice,
};

use crate::{
    ledger::{EmptyResult, SPLIT_FACTOR},
    mmap::mmap,
    sbrk,
    util::{align, extra_brk, MIN_ALIGN},
};

#[derive(Debug, Eq, PartialEq)]
pub struct Block {
    pub size: usize,
    pub data: NonNull<u8>,
    pub free: bool,
}

/// Compare the blocks address.
impl PartialOrd for Block {
    #[inline]
    fn partial_cmp(&self, other: &Block) -> Option<cmp::Ordering> {
        self.data.as_ptr().partial_cmp(&other.data.as_ptr())
    }
}

/// Compare the blocks address.
impl Ord for Block {
    #[inline]
    fn cmp(&self, other: &Block) -> cmp::Ordering {
        self.data.as_ptr().cmp(&other.data.as_ptr())
    }
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

    /// Returns a pointer to the data that this block represents.
    ///
    /// TODO: should this null check?
    pub fn raw_data(&self) -> *mut u8 {
        unsafe { self.data.as_ptr() }
    }

    pub fn merge_right(&mut self, next: Block) -> EmptyResult {
        if !next.is_null() {
            // TODO make this config-able?
            // Zero out the bytes of the next block before we merge
            unsafe {
                ptr::write_bytes(next.data.as_ptr(), 0, next.size);
            }
        }
        self.size += next.size;
        EmptyResult::Ok
    }

    pub fn split(&mut self, size: usize) -> Block {
        // Since this is aligned to 4 bytes there will never be a remainder
        // let new_size = self.size >> 1;
        let new_size = self.size / SPLIT_FACTOR;

        // but just incase
        assert_eq!(self.size % 2, 0);
        assert!(new_size >= size);

        let new_blk = unsafe { Block::new(self.data.as_ptr().add(new_size), new_size) };
        self.size = new_size;

        new_blk
    }
}
