#![feature(allocator_api, asm, llvm_asm, nonnull_slice_from_raw_parts)]
#![allow(unused)]

mod brk;
mod sc;

use core::{
    alloc::{AllocError, AllocRef, GlobalAlloc, Layout, LayoutErr},
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

static mut GLOBAL_BASE: *mut Block = ptr::null_mut();

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
const fn align(size: usize) -> isize {
    ((((size - 1) >> 2) << 2) + 4) as isize
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
#[derive(Copy, Clone, Debug)]
pub struct Block {
    size: usize,
    free: BlockState,
    data: *mut u8,
    next: *mut Block,
}

impl Block {
    ///
    /// # Safety
    /// It ain't
    pub unsafe fn from_raw(ptr: *mut u8, size: usize) -> Self {
        println!("FROM RAW {:?}", ptr);
        println!("blk size {:?}", align(BLOCK_SIZE));

        let mut blk = Self {
            size,
            data: ptr,
            free: BlockState::InUse,
            next: ptr::null_mut(),
        };

        ptr::write(ptr as *mut _, blk);
        blk
    }

    pub fn as_raw(&self) -> *mut Block {
        self.data as *mut Block
    }

    pub fn find_block(mut last: *mut Block, size: usize) -> *mut Block {
        unsafe {
            let mut b = GLOBAL_BASE;
            println!("GB ptr {:?}", b);
            // It's gotta be Some and we keep looping if InUse && our blk is to small
            while !(b.is_null() || (*b).free == BlockState::Free && (*b).size >= size) {
                last = b;
                if (*b).next.is_null() {
                    return b;
                }
                b = (*b).next as *mut _;
            }
            b
        }
    }

    ///
    /// # Safety
    /// It ain't
    pub unsafe fn extend_heap(last: *mut Block, size: usize) -> *mut Block {
        // Returns pointer to the next free chunk
        let mut b = sbrk(align(BLOCK_SIZE))
            .ok()
            .map(|ptr| Block::from_raw(ptr as *mut _, size))
            .unwrap();

        if sbrk(align(BLOCK_SIZE + size)).is_ok() {
            if !last.is_null() {
                println!("RIZAW {:?}", b.as_raw());

                // ptr::write(old.next, b);

                (*last).next = b.data as *mut Block;

                println!("{:?}", last);
            }
            b.data as *mut Block
        } else {
            panic!("NEXT PAGE IS OOM??")
        }
    }
}

///
/// # Safety
/// It ain't
pub unsafe fn malloc(size: usize) -> *mut u8 {
    // This is our first alloc
    if GLOBAL_BASE.is_null() {
        let blk = Block::extend_heap(ptr::null_mut(), size);
        GLOBAL_BASE = blk;
        (*blk).data.add(1)
    } else {
        let blk_ptr = Block::find_block(GLOBAL_BASE, size);
        if blk_ptr.is_null() {
            panic!()
        }

        // TODO
        // We have most likely returned the GLOBAL_BASE block
        // or we need to extend as we have used all our blocks??
        if (*blk_ptr).free == BlockState::InUse {
            println!("GLOBAL_BASE block");
            let new = Block::extend_heap(blk_ptr, size);
            return (*new).data.add(1);
        }

        println!("find block");
        (*blk_ptr).free = BlockState::InUse;
        (*blk_ptr).data.add(1)
    }
}

pub struct Ralloc;

unsafe impl GlobalAlloc for Ralloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        println!("alloc {:?}", layout);
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
    fn alloc(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        Ok(unsafe {
            let ptr = NonNull::new_unchecked(malloc(layout.size()));
            NonNull::slice_from_raw_parts(ptr, layout.size())
        })
    }

    fn alloc_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
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
    ) -> Result<NonNull<[u8]>, AllocError> {
        panic!()
    }
    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        panic!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        unsafe {
            // println!("{:?}", libc::malloc(std::mem::size_of::<u8>()) as *const u8);

            println!("ONE MALLOC {:?}", malloc(512));
            println!("{:?}", (*GLOBAL_BASE));

            println!("TWO MALLOC {:?}", malloc(512));
            println!("{:?}", (*GLOBAL_BASE));

            // ptr::swap((*GLOBAL_BASE).next, )
        }
    }
}
