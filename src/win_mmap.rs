//! Make a request to CreateFileMappingW window's mmap 🎶 🕺🕺 🎶

use core::{mem, ptr};

use crate::syscall;

use winapi::{
    shared::{basetsd::SIZE_T, minwindef::DWORD},
    um::{
        handleapi::{CloseHandle, INVALID_HANDLE_VALUE},
        memoryapi::{
            CreateFileMappingW, FlushViewOfFile, MapViewOfFile, UnmapViewOfFile, VirtualAlloc,
            VirtualProtect, FILE_MAP_ALL_ACCESS, FILE_MAP_COPY, FILE_MAP_EXECUTE, FILE_MAP_READ,
            FILE_MAP_WRITE,
        },
        sysinfoapi::GetSystemInfo,
        winnt::{
            MEM_COMMIT, MEM_LARGE_PAGES, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READ,
            PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_NOACCESS, PAGE_READONLY,
            PAGE_READWRITE, PAGE_WRITECOPY,
        },
    },
};

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
        let new = _mmap(size as usize)?;
        // TODO a check that new == current + size
        // If and only if we keep track of space after first
        // call to VirtualAlloc
        if new != !0 as *const _ {
            self.current = new;
            Ok(new)
        } else {
            // mmap returns !0 on a failed mapping request
            Err(())
        }
    }
}

pub unsafe fn _mmap(size: usize) -> Result<*const u8, ()> {
    let mut info = mem::zeroed();
    GetSystemInfo(&mut info);
    // println!("{}", info.dwPageSize);

    let ptr = VirtualAlloc(
        ptr::null_mut(),
        size as SIZE_T,
        MEM_RESERVE | MEM_COMMIT,
        PAGE_READWRITE,
    );

    if !ptr.is_null() {
        Ok(ptr as *mut u8)
    } else {
        Err(())
    }
}

#[test]
fn mmap_call() {
    unsafe {
        let ptr = _mmap(1024).unwrap() as *mut u8;
        println!("{:?}", ptr);
        let ptr2 = _mmap(1024).unwrap();
        println!("{:?}", 840 * mem::size_of::<u32>());

        for i in 0..1025 {
            // print!(" {} ", i);
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
