#![feature(test)]

extern crate test;

use std::{
    alloc::{GlobalAlloc, Layout},
    ptr,
};

use test::Bencher;

use ralloc::Ralloc as Global;

#[test]
fn allocate_zeroed() {
    unsafe {
        let layout = Layout::from_size_align(1024, 1).unwrap();
        let ptr = Global.alloc_zeroed(layout);

        let mut i = ptr::NonNull::new(ptr).unwrap().as_ptr();
        let end = i.add(layout.size());
        while i < end {
            assert_eq!(*i, 0);
            i = i.offset(1);
        }
        Global.dealloc(ptr, layout);
    }
}

#[bench]
#[cfg_attr(miri, ignore)] // isolated Miri does not support benchmarks
fn alloc_owned_small(b: &mut Bencher) {
    b.iter(|| {
        let _: i32 = unsafe {
            let ptr = Global.alloc(Layout::new::<i32>()) as *mut i32;
            ptr::write(ptr, 10);
            *ptr
        };
    })
}
