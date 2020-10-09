#![feature(allocator_api, asm, llvm_asm, nonnull_slice_from_raw_parts)]
#![allow(unused)]

mod block;
mod breaks;
mod sc;
mod util;

use core::{
    alloc::{AllocError, AllocRef, GlobalAlloc, Layout, LayoutErr},
    cmp, fmt, mem,
    ptr::{self, NonNull},
};

use block::{Block, BlockState};
use breaks::{brk, sbrk};
use sc as syscall;
use util::{align, MIN_ALIGN};

static mut GLOBAL_BASE: *mut Block = ptr::null_mut();

///
/// # Safety
/// It ain't but I'm working on it.
unsafe fn free(ptr: *mut u8, layout: Layout) {
    let mut blk = Block::get_block(ptr);
    (*blk).free = BlockState::Free;

    // Can we combine the previous block with the "current" block
    if !(*blk).prev.is_null() && (*(*blk).prev).free == BlockState::Free {
        blk = Block::absorb((*blk).prev);
    }

    // Can we combine the next block with "current"
    if !(*blk).next.is_null() {
        Block::absorb(blk);
    } else {
        if !(*blk).prev.is_null() {
            (*(*blk).prev).next = ptr::null_mut();
        } else {
            dbg!(&(*GLOBAL_BASE));
            GLOBAL_BASE = ptr::null_mut();
        }
        // Reset the end of the heap to the last block we have
        brk(blk as *const u8);
    }
}

///
/// # Safety
/// It ain't but I'm working on it.
unsafe fn malloc(layout: Layout) -> *mut u8 {
    let size = layout.size();
    // This is our first alloc
    if GLOBAL_BASE.is_null() {
        let blk = Block::extend_heap(ptr::null_mut(), size);
        GLOBAL_BASE = blk;
        (*blk).data.add(1) as *mut u8
    } else {
        let blk_ptr = Block::find_block(GLOBAL_BASE, size);
        if blk_ptr.is_null() {
            panic!("Found null pointer")
        }

        dbg!(*blk_ptr);
        dbg!(size);

        let blk_size = (*blk_ptr).size;
        if ((blk_size as isize - size as isize) >= (block::BLOCK_SIZE + 4) as isize) {
            Block::split_block(blk_ptr, size);
        }

        // We need to extend the heap
        if (*blk_ptr).free == BlockState::InUse {
            let new = Block::extend_heap(blk_ptr, size);
            return (*new).data.add(1) as *mut u8;
        }

        (*blk_ptr).free = BlockState::InUse;
        (*blk_ptr).data.add(1) as *mut u8
    }
}

// TODO
unsafe fn align_malloc(layout: Layout) -> *mut u8 {
    let mut out = ptr::null_mut();
    let align = layout.align().max(crate::mem::size_of::<usize>());
    let ret = libc::posix_memalign(&mut out, align, layout.size());
    if ret != 0 {
        ptr::null_mut()
    } else {
        out as *mut u8
    }
}

pub struct Ralloc;

unsafe impl GlobalAlloc for Ralloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // dbg!(&layout);
        eprintln!("ALLOC {}", layout.size());
        if layout.align() <= MIN_ALIGN && layout.align() <= layout.size() {
            malloc(layout)
        } else {
            // ALIGN we have a large enough value to
            dbg!(malloc(layout))
        }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = self.alloc(layout);
        if !ptr.is_null() {
            ptr::write_bytes(ptr, 0, layout.size());
        }
        ptr
    }

    // unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    //     libc::realloc(ptr as *mut std::ffi::c_void, layout.size()) as *mut u8
    // }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        free(ptr, layout)
    }
}

unsafe impl AllocRef for &Ralloc {
    fn alloc(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        Ok(unsafe {
            let ptr = NonNull::new_unchecked(malloc(layout));
            NonNull::slice_from_raw_parts(ptr, layout.size())
        })
    }

    fn alloc_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        println!("{:?}", layout);
        Ok(unsafe {
            let ptr = NonNull::new_unchecked(malloc(layout));
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
            println!("ONE MALLOC {:?}", malloc(Layout::new::<u32>()));
            println!("{:?}", (*GLOBAL_BASE));

            println!("TWO MALLOC {:?}", malloc(Layout::new::<u32>()));
            println!("{:?}", (*GLOBAL_BASE));
        }
    }
}
