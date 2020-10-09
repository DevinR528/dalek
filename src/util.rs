use std::{cmp, mem, ptr};

// The minimum alignment guaranteed by the architecture. This value is used to
// add fast paths for low alignment values.
#[cfg(all(any(
    target_arch = "arm",
    target_arch = "mips",
    target_arch = "mipsel",
    target_arch = "powerpc"
)))]
pub const MIN_ALIGN: usize = 8;

// The minimum alignment guaranteed by the architecture. This value is used to
// add fast paths for low alignment values.
#[cfg(all(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "powerpc64",
    target_arch = "powerpc64le",
    target_arch = "mips64",
    target_arch = "s390x",
    target_arch = "sparc64"
)))]
pub const MIN_ALIGN: usize = 16;

#[inline]
pub fn extra_brk(size: usize) -> usize {
    // TODO: Tweak this.
    /// The BRK multiplier.
    ///
    /// The factor determining the linear dependence between the minimum segment, and the acquired
    /// segment.
    const MULTIPLIER: usize = 2;
    /// The minimum extra size to be BRK'd.
    const MIN_EXTRA: usize = 1024;
    /// The maximal amount of _extra_ bytes.
    const MAX_EXTRA: usize = 65536;

    cmp::max(MIN_EXTRA, cmp::min(MULTIPLIER * size, MAX_EXTRA))
}

/// TODO this algo aligns to 32 bit ptr sizes; use `size_of::<usize>()` somehow to fix.
pub const fn align(size: usize) -> isize {
    ((((size - 1) >> 2) << 2) + 4) as isize
}
