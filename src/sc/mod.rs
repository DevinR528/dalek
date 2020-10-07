pub mod sys_num;

#[inline(always)]
pub unsafe fn syscall0(n: usize) -> usize {
    let ret: usize;
    llvm_asm!("syscall" : "={rax}"(ret)
                   : "{rax}"(n)
                   : "rcx", "r11", "memory"
                   : "volatile");
    ret
}

#[inline(always)]
pub unsafe fn syscall1(n: usize, a1: usize) -> usize {
    let ret: usize;
    llvm_asm!("syscall": "={rax}"(ret)
        : "{rax}"(n), "{rdi}"(a1)
        : "rcx", "r11", "memory"
        : "volatile"
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall2(n: usize, a1: usize, a2: usize) -> usize {
    let ret: usize;
    llvm_asm!("syscall" : "={rax}"(ret)
        : "{rax}"(n), "{rdi}"(a1), "{rsi}"(a2)
        : "rcx", "r11", "memory"
        : "volatile"
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall3(n: usize, a1: usize, a2: usize, a3: usize) -> usize {
    let ret: usize;
    llvm_asm!("syscall" : "={rax}"(ret)
        : "{rax}"(n), "{rdi}"(a1), "{rsi}"(a2), "{rdx}"(a3)
        : "rcx", "r11", "memory"
        : "volatile"
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall4(n: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> usize {
    let ret: usize;
    llvm_asm!("syscall" : "={rax}"(ret)
        : "{rax}"(n), "{rdi}"(a1), "{rsi}"(a2), "{rdx}"(a3),
          "{r10}"(a4)
        : "rcx", "r11", "memory"
        : "volatile"
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall5(n: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize) -> usize {
    let ret: usize;
    llvm_asm!("syscall" : "={rax}"(ret)
        : "{rax}"(n), "{rdi}"(a1), "{rsi}"(a2), "{rdx}"(a3),
          "{r10}"(a4), "{r8}"(a5)
        : "rcx", "r11", "memory"
        : "volatile"
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall6(
    n: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
) -> usize {
    let ret: usize;
    llvm_asm!("syscall" : "={rax}"(ret)
        : "{rax}"(n), "{rdi}"(a1), "{rsi}"(a2), "{rdx}"(a3),
          "{r10}"(a4), "{r8}"(a5), "{r9}"(a6)
        : "rcx", "r11", "memory"
        : "volatile"
    );
    ret
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub unsafe fn syscall7(
    n: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
    a7: usize,
) -> usize {
    let ret: usize;
    llvm_asm!("syscall" : "={rax}"(ret)
        : "{rax}"(n), "{rdi}"(a1), "{rsi}"(a2), "{rdx}"(a3),
          "{r10}"(a4), "{r8}"(a5), "{r9}"(a6), "{r10}"(a7)
        : "rcx", "r11", "memory"
        : "volatile"
    );
    ret
}

#[macro_export]
macro_rules! syscall {
    ($nr:ident) => {
        $crate::sc::syscall0($crate::sc::sys_num::$nr)
    };

    ($nr:ident, $a1:expr) => {
        $crate::sc::syscall1($crate::sc::sys_num::$nr, $a1 as usize)
    };

    ($nr:ident, $a1:expr, $a2:expr) => {
        $crate::syscalls::syscall2($crate::sc::sys_num::$nr, $a1 as usize, $a2 as usize)
    };

    ($nr:ident, $a1:expr, $a2:expr, $a3:expr) => {
        $crate::syscalls::syscall3(
            $crate::sc::sys_num::$nr,
            $a1 as usize,
            $a2 as usize,
            $a3 as usize,
        )
    };

    ($nr:ident, $a1:expr, $a2:expr, $a3:expr, $a4:expr) => {
        $crate::syscalls::syscall4(
            $crate::sc::sys_num::$nr,
            $a1 as usize,
            $a2 as usize,
            $a3 as usize,
            $a4 as usize,
        )
    };

    ($nr:ident, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr) => {
        $crate::syscalls::syscall5(
            $crate::sc::sys_num::$nr,
            $a1 as usize,
            $a2 as usize,
            $a3 as usize,
            $a4 as usize,
            $a5 as usize,
        )
    };

    ($nr:ident, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr, $a6:expr) => {
        $crate::syscalls::syscall6(
            $crate::sc::sys_num::$nr,
            $a1 as usize,
            $a2 as usize,
            $a3 as usize,
            $a4 as usize,
            $a5 as usize,
            $a6 as usize,
        )
    };

    ($nr:ident, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr, $a6:expr, $a7:expr) => {
        $crate::syscalls::syscall7(
            $crate::sc::sys_num::$nr,
            $a1 as usize,
            $a2 as usize,
            $a3 as usize,
            $a4 as usize,
            $a5 as usize,
            $a6 as usize,
            $a7 as usize,
        )
    };
}
