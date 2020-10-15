//! Make a request to mmap 🎶 🕺🕺 🎶

use core::ptr;

use crate::syscall;

use libc::{MAP_FAILED, _SC_PAGESIZE, _SC_PAGE_SIZE};

/// This memory can be read.
/// Sets the permissions to allow reading.
const PROT_READ: u8 = 1;
/// Sets the permissions so the memory can be written to.
const PROT_WRITE: u8 = 2;
/// Share this memory with all other processes. Changes will
/// be written back to memory from other procs.
const MAP_SHARED: u8 = 0x0001;
/// This means the memory is not connected to a file.
const MAP_ANON: u8 = 0x0020;
/// Do not share this memory with other processes, changes
/// will __not__ be written back to memory.
const MAP_PRIVATE: u8 = 0x0002;

#[cfg(target_arch = "x86_64")]
/// Default non big page size on linux x86_64.
pub const PAGE_SIZE: usize = 4096;

#[cfg(target_arch = "mips")]
/// Default non big page size on mips.
pub const PAGE_SIZE: usize = 16384;

// TODO docs and such
const STACK: u8 = 0;
const OFFSET: u64 = 0;
const NOT_FILE: i8 = -1;

static mut PAGE: MMap = MMap {
    current: ptr::null(),
};

// TODO meaning full error (it's oom or nothing)
/// The size of the requested allocation.
///
/// This must include the `ralloc::Block` size and any other meta data/optimization stuff.
pub unsafe fn mmap(size: isize) -> Result<*const u8, ()> {
    PAGE.mmap(size)
}

struct MMap {
    current: *const u8,
}

unsafe impl Send for MMap {}
unsafe impl Sync for MMap {}

impl MMap {
    unsafe fn mmap(&mut self, size: isize) -> Result<*const u8, ()> {
        let new = _mmap(size as usize);
        if new != !0 as *const _ {
            self.current = new;
            Ok(new)
        } else {
            // mmap returns !0 on a failed mapping request
            Err(())
        }
    }
}

// #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub unsafe fn _mmap(size: usize) -> *const u8 {
    syscall!(
        MMAP,
        ptr::null() as *const u8,
        size,
        PROT_READ | PROT_WRITE,
        MAP_SHARED | MAP_ANON | STACK,
        NOT_FILE,
        OFFSET
    ) as *const u8
}

#[test]
fn mmap_call() {
    unsafe {
        println!("{}", libc::sysconf(_SC_PAGESIZE));

        let ptr = _mmap(10) as *mut u8;
        println!("{:?}", ptr);
        let ptr2 = _mmap(1024) as *mut u8;
        println!("{:?}", ptr2);

        for i in 0..1025 {
            ptr::write(ptr.add(i), 1);
        }

        // mmap allocates a PAGE_SIZE multiple of size, so under 4096 is a page
        // 4096 + 1 is 2 pages or 8182 bytes
        for i in 0..4096 {
            // page size must be this at 4097 it crashes
            let x = ptr::read(ptr.add(i));

            // up to but not including
            if i < 1025 {
                assert_eq!(x, 1, "{}", i);
            } else {
                assert_eq!(x, 0, "{}", i);
            }
        }
    }
}
