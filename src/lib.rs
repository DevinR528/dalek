#![feature(allocator_api, asm, llvm_asm, nonnull_slice_from_raw_parts)]
#![allow(unused)]

mod block;
mod breaks;
mod mmap;
mod pointer;
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
///
/// All alignment and size calculation is done in `Block::extend_heap` so
/// if and aligned pointer is needed you must do it again.
/// FIXME the above should be encapsulated.
unsafe fn malloc(layout: Layout) -> *mut u8 {
    let size = layout.size();
    // This is our first alloc
    if GLOBAL_BASE.is_null() {
        let blk = Block::extend_heap(ptr::null_mut(), size);
        GLOBAL_BASE = blk;
        (*blk).data.add(1) as *mut u8
    } else {
        // watch this when fixing ptr arithmetic this size is the data size not total
        let blk_ptr = Block::find_block(GLOBAL_BASE, size);
        if blk_ptr.is_null() {
            panic!("Found null pointer")
        }

        // dbg!(*blk_ptr);
        // dbg!(size);

        // PTR MATH fix
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

unsafe fn realloc(ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    eprintln!("REALLOC {:?} {:?}", ptr, layout);
    // SAFETY: the caller must ensure that the `new_size` does not overflow.
    // `layout.align()` comes from a `Layout` and is thus guaranteed to be valid.
    let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
    // SAFETY: the caller must ensure that `new_layout` is greater than zero.
    let new_ptr = malloc(new_layout);

    if !new_ptr.is_null() {
        // SAFETY: the previously allocated block cannot overlap the newly allocated block.
        // The safety contract for `dealloc` must be upheld by the caller.
        ptr::copy_nonoverlapping(ptr, new_ptr, cmp::min(layout.size(), new_size));
        // Block::copy_block(
        //     // This is dumb probably should just use copy and not convert u8 -> Block -> u8
        //     ptr.cast::<Block>().offset(-1), // We have a ptr to the end of `Block`, back it up
        //     new_ptr.cast::<Block>().offset(-1), // Same here jump back to `Block`
        //     new_size,
        // );
        free(ptr, layout);
    }
    dbg!(*GLOBAL_BASE);
    dbg!(*Block::get_block(new_ptr));
    new_ptr
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

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        realloc(ptr, layout, new_size)
    }

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
            println!("ONE MALLOC {:?}", malloc(Layout::new::<usize>()));
            println!("{:#?}", (*GLOBAL_BASE));

            println!("TWO MALLOC {:?}", malloc(Layout::new::<u32>()));
            println!("{:?}", (*GLOBAL_BASE));
        }
    }
}
