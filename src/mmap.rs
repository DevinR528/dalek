//! Make a request to mmap 🎶 🕺🕺 🎶

use core::ptr;

use crate::syscall;

use libc::PT_DYNAMIC;

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
        let old = self.current_page();
        let expect = old.clone().offset(size);

        let new = _mmap(expect, size as usize);

        if expect == new {
            self.current = expect;
            Ok(old)
        } else {
            // BRK failed. This syscall is rather weird, but whenever it fails (e.g. OOM) it
            // returns the old (unchanged) break.
            assert_eq!(old, new);
            Err(())
        }
    }

    fn current_page(&mut self) -> *const u8 {
        if !self.current.is_null() {
            let res = self.current;
            debug_assert!(
                res == current_page(),
                "The cached program break is out of sync with the \
                 actual program break. Are you interfering with BRK? If so, prefer the \
                 provided 'sbrk' instead, then."
            );
            return res;
        }

        let cur = current_page();
        self.current = cur;
        cur
    }
}

fn current_page() -> *const u8 {
    unsafe { _mmap(ptr::null(), 1024) }
}

// #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub unsafe fn _mmap(ptr: *const u8, size: usize) -> *const u8 {
    syscall!(
        MMAP,
        ptr,
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
        let ptr = _mmap(ptr::null(), 1024) as *mut u8;
        // println!("{:?}", ptr);
        let ptr2 = _mmap(ptr::null(), 1024) as *mut u8;
        // println!("{:?}", ptr2);

        // for i in 0..1025 {
        //     ptr::write(ptr.add(i), 0);
        // }

        // for i in 0..4096 {
        //     // page size must be this at 4097 it crashes
        //     println!("{:?}", i);
        //     let x = ptr::read(ptr.add(i));
        // }
    }
}
