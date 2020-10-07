#![feature(allocator_api)]

use core::{
    alloc::{AllocErr, AllocRef, GlobalAlloc, Layout, LayoutErr},
    ptr,
};

pub const fn align(size: usize) -> usize {
    (((size - 1) >> 2) << 2) + 4
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockState {
    InUse,
    Free,
}

pub struct Block {
    size: usize,
    next: *const Block,
    free: BlockState,
}

impl Block {
    pub fn find_block(last: Block, size: usize) -> Option<Block> {
        let b: Option<Block> = todo!();
        while (!b.is_some() && (b.unwrap().free == BlockState::InUse && b.unwrap().size >= size)) {
            last = b.unwrap();
            b = Some(unsafe { ptr::read(b.unwrap().next) });
        }
        b
    }
}

pub struct Ralloc {
    pub mmap: u8,
}

unsafe impl GlobalAlloc for Ralloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        panic!("its a start")
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        panic!("its a start")
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        panic!("its a start")
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        panic!("its a start")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        println!("{}", (((9 - 1) >> 2) << 2) + 4);
    }
}
