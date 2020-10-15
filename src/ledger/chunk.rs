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

    fn ceil_log_2(d: usize) -> usize {
        uint32_t result = is_power_of_two(d) ? 0 : 1;
        while (d>1) {
          result++;
          d = d>>1;
        }
        return result;
      }
      
      fn calculate_shift_magic(d: usize) -> usize {
        if (d > chunksize) {
          return 1;
        } else if (is_power_of_two(d)) {
          return ceil_log_2(d);
        } else {
          return 32+ceil_log_2(d);
        }
      }

      
      fn calculate_multiply_magic(d: usize) -> usize {
        if (d > chunksize) {
          return 1;
        } else if (is_power_of_two(d)) {
          return 1;
        } else {
          return (d-1+(1ul << calculate_shift_magic(d)))/d;
        }
      }
      
      fn gcd(a: usize, uint64_t b) -> usize {
        if (a==b) return a;
        if (a<b) return gcd(a, b-a);
        return gcd(b, a-b);
      }
      
      fn lcm(a: usize, uint64_t b) -> usize {
        uint64_t g = gcd(a, b);
        return (a/g)*b;
      }
      
      fn calculate_foliosize(objsize: usize) -> usize {
        if (objsize > chunksize) return objsize;
        if (is_power_of_two(objsize)) {
          if (objsize < pagesize) return pagesize;
          else return objsize;
        }
        if (objsize > 16*1024) return objsize;
        if (objsize > 256) {
          return (objsize/cacheline_size)*pagesize;
        }
        if (objsize > pagesize) return objsize;
        return lcm(objsize, pagesize);
      }

    #[test]
    fn math_stuff() {

    }

}