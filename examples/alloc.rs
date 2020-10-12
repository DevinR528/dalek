#![feature(allocator_api, llvm_asm)]

use std::ptr;

use ralloc::Ralloc;

#[global_allocator]
static GLOBAL: Ralloc = Ralloc;

fn main() {
    {
        let x = vec![1, 2, 3];
        assert!(!x.is_empty())
    }
    // unsafe {
    //     {
    //         let ptr = std::alloc::alloc(std::alloc::Layout::new::<u32>());
    //         assert!(!ptr.is_null());
    //         ptr::write(ptr, 10);
    //         assert_eq!(10, (*ptr));
    //     }
    //     {
    //         let ptr = std::alloc::alloc(std::alloc::Layout::new::<u32>());
    //         assert!(!ptr.is_null());
    //         ptr::write(ptr, 10);
    //         assert_eq!(10, (*ptr));
    //     }
    // }
}
