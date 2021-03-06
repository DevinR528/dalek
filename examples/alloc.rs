#![no_std]
#![feature(allocator_api, llvm_asm)]

extern crate alloc;

use alloc::vec::Vec;
use core::ptr;

use ralloc::Ralloc;

#[global_allocator]
static GLOBAL: Ralloc = Ralloc;

fn main() {
    let mut v = Vec::new();
    v.push(1u32);
    v.push(2);
    v.push(3);
    // assert!(!v.is_empty());

    // unsafe {
    //     {
    //         let ptr = std::alloc::alloc(std::alloc::Layout::new::<u32>());
    //         assert!(!ptr.is_null());
    //         ptr::write(ptr, 10);
    //         assert_eq!(10, (*ptr));
    //         ptr::drop_in_place(ptr);
    //     }
    //     {
    //         let ptr = std::alloc::alloc(std::alloc::Layout::new::<u32>());
    //         assert!(!ptr.is_null());
    //         ptr::write(ptr, 10);
    //         assert_eq!(10, (*ptr));
    //         ptr::drop_in_place(ptr);
    //     }
    // }
}
