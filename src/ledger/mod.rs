pub mod block;
pub mod bookkeeper;
mod chunk;
pub mod raw_slice;

pub use block::Block;
pub use bookkeeper::BookKeeper;
pub use raw_slice::RawSlice;

pub const SPLIT_FACTOR: usize = 2;

/// The size of our `Block` aligned to 4 bytes.
pub const BLOCK_ALIGN: usize = crate::util::align(std::mem::size_of::<Block>()) as usize;

pub enum EmptyResult {
    Ok,
    Err,
}

impl EmptyResult {
    #[inline]
    pub fn is_err(&self) -> bool {
        matches!(self, Self::Err)
    }

    #[inline]
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    #[inline]
    pub fn unwrap(self) {
        if let Self::Err = self {
            panic!("called `unwrap` on an error value")
        }
    }
}
