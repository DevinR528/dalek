use core::{
    alloc::{AllocError, AllocRef, GlobalAlloc, Layout, LayoutErr},
    cmp, fmt, mem,
    ptr::{self, NonNull},
};

use crate::{
    breaks::{brk, sbrk},
    pointer::Pointer,
    sc as syscall,
    util::{align, MIN_ALIGN},
};

/// IMPORTANT the size of meta data.
///
/// ```notrust
/// |---------|______________________________|
///   metadata        the space requested
/// ```
pub const BLOCK_SIZE: usize = align(mem::size_of::<Block>()) as usize;

/// The state of the blocks data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockState {
    /// The program is using this chunk of memory.
    InUse,
    /// The chunk of memory has been deallocated.
    Free,
}

/// ,___,<br>
/// {O,o}<br>
/// |)``)<br>
/// HOOTIE!!<br>
// TODO make accessors or do it right and make a proper Pointer type
// to wrap *mut/const still need accessors though.
#[derive(Clone)]
pub struct Block {
    pub size: usize,
    pub free: BlockState,
    pub data: Pointer<Block>,
    pub next: Pointer<Block>,
    pub prev: Pointer<Block>,
}

impl fmt::Debug for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Block")
            .field("size", &self.size)
            .field("free", &self.free)
            .field("data", &self.data)
            .field(
                "next",
                if self.next.is_null() {
                    &"null"
                } else {
                    unsafe { &*self.next.get() }
                },
            )
            .field(
                "prev",
                if self.prev.is_null() {
                    &"null"
                } else {
                    &self.prev
                    // unsafe { &(*self.prev) }
                },
            )
            .finish()
    }
}

impl Block {
    pub fn as_raw(&self) -> *mut Block {
        self.data.get()
    }

    ///
    /// # Safety
    /// It ain't
    pub unsafe fn from_raw(ptr: *mut u8, size: usize, prev: Pointer<Block>) -> Self {
        let mut blk = Self {
            size,
            data: Pointer::new(ptr as *mut Block),
            free: BlockState::InUse,
            next: Pointer::empty(),
            prev,
        };
        ptr::write(ptr as *mut _, blk.clone());
        blk
    }

    pub fn find_block(mut last: *mut Block, size: usize) -> Pointer<Block> {
        unsafe {
            let mut b = crate::GLOBAL_BASE;

            if b.is_null() {
                return Pointer::empty();
            }
            let mut b = Pointer::new(b);
            // println!("GB ptr {:?}", b);
            // It's gotta be Some and we keep looping if InUse && our blk is to small
            while !(b.is_null() || b.is_free() && b.size() >= size) {
                if b.next().is_null() {
                    return b;
                }
                b = b.next();
            }
            b
        }
    }

    /// Shift the heap break and create a new `Block` at the beginning of the break.
    ///
    /// The `Block` that is returned is assumed in use, not free.
    /// # Safety
    /// It ain't
    pub unsafe fn extend_heap(last: Pointer<Block>, size: usize) -> Pointer<Block> {
        let need_size = align(BLOCK_SIZE + size);
        let size = need_size as usize - BLOCK_SIZE;
        // Returns pointer to the next free chunk
        let mut b = sbrk(BLOCK_SIZE as isize)
            .ok()
            // `Block::from_raw` assumes the Block is InUse
            .map(|ptr| Block::from_raw(ptr as *mut _, size, last.clone()))
            .unwrap();

        if sbrk(need_size).is_ok() {
            if !last.is_null() {
                (*last.get()).next = b.data.clone();
            }
            b.data
        } else {
            panic!("NEXT PAGE IS OOM??")
        }
    }

    ///
    /// # Safety
    /// It ain't
    pub unsafe fn get_block(find_ptr: *mut u8) -> Pointer<Block> {
        // TODO validate ptr?
        Pointer::new(find_ptr.cast::<Block>().offset(-1))
    }

    /// Merge the `next` block with current and set `next`s `prev` pointer to
    /// current
    /// # Safety
    /// It ain't
    pub unsafe fn absorb(ptr: Pointer<Block>) -> Pointer<Block> {
        if !ptr.is_null() {
            // let mut blk = *ptr;
            // If we have a non null and free block absorb it
            if !ptr.is_null() && ptr.is_free() {
                ptr.add_size(BLOCK_SIZE + ptr.next().size());
                // (*ptr).size += BLOCK_SIZE + (*(*ptr).next).size;
                // (*ptr).next = (*(*ptr).next).next;
                ptr.set_next(ptr.next().next());

                // Now set "current" to prev for the newly "next" blk
                if !ptr.next().is_null() {
                    ptr.next().set_prev(ptr.clone());
                }
                dbg!(&*ptr.get());
            }
        }
        ptr
    }

    /// Take the existing Block and split it adding the new block after the
    /// existing one. The data is kept in `ptr`, `new` will be `BlockState::Free`.
    ///
    /// # Safety
    /// * `ptr`'s `Block.size` must be larger than `size + BLOCK_SIZE`
    pub unsafe fn split_block(ptr: Pointer<Block>, size: usize) {
        let new = Pointer::new(
            Block::from_raw(
                ptr.cast::<u8>().add(size + BLOCK_SIZE).get(),
                ptr.size() - size - BLOCK_SIZE, // This is probably wrong also above
                ptr.clone(),
            )
            .as_raw(),
        );
        // New's next is the old ptr's next
        new.set_next(ptr.next());
        // Since we are not filling the new block mark it as free
        new.mark_free();

        ptr.set_size(size);
        // new is Block.data (pointer to itself) so this works
        ptr.set_next(new);
        dbg!(&*crate::GLOBAL_BASE);
        dbg!(size);
    }

    pub unsafe fn copy_block(src: *mut Block, dst: *mut Block, count: usize) {
        ptr::copy_nonoverlapping(src.add(1).cast::<u8>(), dst.add(1).cast::<u8>(), count)
    }
}
