use std::{ptr, sync::Mutex};

use crate::syscall;

static mut BRK: BrkState = BrkState {
    current: ptr::null(),
};

pub unsafe fn sbrk(ptr: *const u8) -> *const u8 {
    BRK.sbrk(ptr as isize)
        .expect("BreakState has gotten out of sync with memory")
}

struct BrkState {
    current: *const u8,
}

unsafe impl Send for BrkState {}
unsafe impl Sync for BrkState {}

impl BrkState {
    unsafe fn sbrk(&mut self, size: isize) -> Result<*const u8, ()> {
        let old = self.current_brk();
        let expect = old.clone().offset(size);

        let new = brk(expect);

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

    fn current_brk(&mut self) -> *const u8 {
        if !self.current.is_null() {
            let res = self.current;
            debug_assert!(
                res == current_brk(),
                "The cached program break is out of sync with the \
                 actual program break. Are you interfering with BRK? If so, prefer the \
                 provided 'sbrk' instead, then."
            );
            return res;
        }

        let cur = current_brk();
        self.current = cur;
        cur
    }
}

fn current_brk() -> *const u8 {
    unsafe { brk(ptr::null()) }
}

// #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub unsafe fn brk(ptr: *const u8) -> *const u8 {
    syscall!(BRK, ptr) as *const u8
}
