#![feature(allocator_api, llvm_asm)]

use ralloc::Ralloc;

#[global_allocator]
static GLOBAL: Ralloc = Ralloc;

fn main() {
    unsafe { assert!(!std::alloc::alloc(std::alloc::Layout::new::<u32>()).is_null()) }
}
