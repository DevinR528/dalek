#![feature(allocator_api, asm, llvm_asm, nonnull_slice_from_raw_parts)]
#![allow(unused)]

mod brk;
mod sc;

use core::{
    alloc::{AllocErr, AllocRef, GlobalAlloc, Layout, LayoutErr},
    cmp,
    ptr::{self, NonNull},
};

use brk::{brk, sbrk};
use sc as syscall;

/// IMPORTANT the size of meta data.
///
/// ```notrust
/// |---------|______________________________|
///   metadata        the space requested
/// ```
const BLOCK_SIZE: usize = std::mem::size_of::<Block>();

static mut GLOBAL_BASE: Option<Block> = None;

#[inline]
pub fn extra_brk(size: usize) -> usize {
    // TODO: Tweak this.
    /// The BRK multiplier.
    ///
    /// The factor determining the linear dependence between the minimum segment, and the acquired
    /// segment.
    const MULTIPLIER: usize = 2;
    /// The minimum extra size to be BRK'd.
    const MIN_EXTRA: usize = 1024;
    /// The maximal amount of _extra_ bytes.
    const MAX_EXTRA: usize = 65536;

    cmp::max(MIN_EXTRA, cmp::min(MULTIPLIER * size, MAX_EXTRA))
}

/// TODO this algo aligns to 32 bit ptr sizes; use `size_of::<usize>()` somehow to fix.
const fn align(size: usize) -> usize {
    (((size - 1) >> 2) << 2) + 4
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockState {
    InUse,
    Free,
}

/// ,___,<br>
/// {O,o}<br>
/// |)``)<br>
/// HOOTIE!!<br>
#[derive(Clone, Copy, Debug)]
pub struct Block {
    size: usize,
    free: BlockState,
    data: *mut u8,
    next: *const Block,
}

impl Block {
    pub fn from_raw(ptr: *const u8, size: usize) -> Self {
        Self {
            size,
            data: ptr as *mut u8,
            free: BlockState::InUse,
            next: ptr::null(),
        }
    }

    pub fn as_raw(&self) -> *mut Block {
        self as *const _ as *mut _
    }

    pub fn find_block(mut last: Block, size: usize) -> Option<Block> {
        let mut b: Option<Block> = unsafe { GLOBAL_BASE };
        while (b.is_some() && !(b.unwrap().free == BlockState::InUse && b.unwrap().size >= size)) {
            last = b.unwrap();
            b = Some(unsafe { ptr::read(b.unwrap().next) });
        }
        b
    }

    pub fn extend_heap(last: Option<Block>, size: usize) -> Block {
        let mut b = unsafe { sbrk(0).ok().map(|ptr| Block::from_raw(ptr, size)) }; // srbk(0) // returns the first heap ptr
        unsafe {
            if let Ok(blk) = sbrk((BLOCK_SIZE + size) as isize) {
                if let Some(mut old) = last {
                    old.next = b.map(|blk| blk.as_raw()).unwrap_or_else(ptr::null_mut)
                }
                b.unwrap()
            } else {
                panic!("NEXT PAGE IS OOM??")
            }
        }
    }
}

///
/// # Safety
pub unsafe fn malloc(size: usize) -> *mut u8 {
    // This is our first alloc
    if GLOBAL_BASE.is_none() {
        let blk = Block::extend_heap(None, size);
        GLOBAL_BASE = Some(blk);
        blk.data.add(BLOCK_SIZE)
    } else if let Some(mut blk) = Block::find_block(GLOBAL_BASE.unwrap(), size) {
        blk.free = BlockState::InUse;
        blk.data.add(BLOCK_SIZE)
    } else {
        panic!("OOM")
    }
}

pub struct Ralloc;

unsafe impl GlobalAlloc for Ralloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        println!("{:?}", layout);
        malloc(layout.size())
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        println!("{:?}", layout);
        malloc(layout.size())
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        println!("{:?}", layout);
        malloc(layout.size())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        println!("{:?}", layout);
        panic!("its a start")
    }
}

unsafe impl AllocRef for &Ralloc {
    fn alloc(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocErr> {
        Ok(unsafe {
            let ptr = NonNull::new_unchecked(malloc(layout.size()));
            NonNull::slice_from_raw_parts(ptr, layout.size())
        })
    }

    fn alloc_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocErr> {
        println!("{:?}", layout);
        Ok(unsafe {
            let ptr = NonNull::new_unchecked(malloc(layout.size()));
            NonNull::slice_from_raw_parts(ptr, layout.size())
        })
    }

    unsafe fn dealloc(&self, ptr: NonNull<u8>, layout: Layout) {
        println!("{:?}", layout);
        panic!("its a start")
    }

    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocErr> {
        panic!()
    }
    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocErr> {
        panic!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        println!("{}", (((9 - 1) >> 2) << 2) + 4);
        println!("{:?}", unsafe { crate::brk::sbrk(0) });
        println!("{:?}", unsafe { Block::extend_heap(None, 10) });
        println!("{:?}", unsafe { malloc(10) });
    }
}
