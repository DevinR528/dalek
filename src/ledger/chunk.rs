use core::{
    cmp, fmt, mem,
    ops::{Deref, DerefMut},
    ptr::{self, NonNull},
    slice,
};

use crate::{
    ledger::{block::Block, raw_slice::RawSlice, EmptyResult, SPLIT_FACTOR},
    mmap::{mmap, PAGE_SIZE},
    sbrk,
    util::{align, extra_brk, MIN_ALIGN},
};

pub const EIGHT_BYTES: usize = 8;
pub const SIXTEEN_BYTES: usize = 16;
pub const THIRTY_TWO_BYTES: usize = 32;
pub const SIXTY_FOUR_BYTES: usize = 64;

pub const fn size_to_class(size: usize) -> usize {
    match size {
        0 => !0,
        1..=EIGHT_BYTES => 8,
        9..=SIXTEEN_BYTES => 16,
        17..=THIRTY_TWO_BYTES => 32,
        33..=SIXTY_FOUR_BYTES => 64,
        _ => !0,
    }
}

#[derive(Debug)]
pub struct Chunk {
    pub size_class: usize,
    pub blks: RawSlice<Block>,
}

impl Chunk {
    /// Create a new `Chunk`.
    ///
    /// Each `Chunk` represents a specific size class.
    pub fn new(data: *mut u8, size_class: usize, page_multiple: usize) -> Self {
        Self {
            size_class,
            blks: unsafe {
                RawSlice::new(
                    data as *mut Block,
                    0,
                    (PAGE_SIZE * page_multiple) / size_to_class(size_class),
                )
            },
        }
    }

    pub fn is_null(&self) -> bool {
        self.blks.as_ptr() == 0x1 as *mut _
    }

    /// Returns a pointer to the data that this block represents.
    ///
    /// TODO: should this null check?
    pub fn raw_data(&self) -> *mut u8 {
        unsafe { self.blks.as_ptr() as *mut _ }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const CHUNK_SIZE: usize = 16;
    const CACHE_LINE: usize = 16;

    fn is_power_of_2(num: usize) -> bool {
        (num & (num - 1)) == 0
    }

    fn ceil_log_2(mut d: usize) -> usize {
        let mut result = if is_power_of_2(d) { 0 } else { 1 };
        while d > 1 {
            result += 1;
            d >>= 1;
        }
        result
    }

    fn calculate_shift_magic(d: usize) -> usize {
        if (d > CHUNK_SIZE) {
            1
        } else if (is_power_of_2(d)) {
            ceil_log_2(d)
        } else {
            32 + ceil_log_2(d)
        }
    }

    fn calculate_multiply_magic(d: usize) -> usize {
        if (d > CHUNK_SIZE) || is_power_of_2(d) {
            1
        } else {
            (d - 1 + (1 << calculate_shift_magic(d))) / d
        }
    }

    fn gcd(a: usize, b: usize) -> usize {
        if (a == b) {
            return a;
        };
        if (a < b) {
            return gcd(a, b - a);
        };
        gcd(b, a - b)
    }

    fn lcm(a: usize, b: usize) -> usize {
        let g = gcd(a, b);
        (a / g) * b
    }

    fn calculate_foliosize(objsize: usize) -> usize {
        if (objsize > CHUNK_SIZE) {
            return objsize;
        };
        if (is_power_of_2(objsize)) {
            return if (objsize < PAGE_SIZE) {
                PAGE_SIZE
            } else {
                objsize
            };
        }
        if (objsize > 16 * 1024) {
            return objsize;
        };
        if (objsize > 256) {
            return (objsize / CACHE_LINE) * PAGE_SIZE;
        }

        if (objsize > PAGE_SIZE) {
            return objsize;
        };
        lcm(objsize, PAGE_SIZE)
    }

    #[test]
    fn math_stuff() {}
}
