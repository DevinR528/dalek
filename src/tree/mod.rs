use core::{
    cell::UnsafeCell,
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

pub enum Balance {
    /// `Left` balanced is equal to -1.
    Left,
    /// `Right` balanced is equal to 1.
    Right,
    /// `Center`ed is equal to 0.
    Center,
}

#[derive(Debug, Eq, PartialEq)]
pub enum NodeColor {
    Red,
    Black,
}

pub struct Node<T> {
    addr: *mut Node<T>,
    item: NonNull<T>,
    color: NodeColor,
    parent: *mut Node<T>,
    left: *mut Node<T>,
    right: *mut Node<T>,
}

impl<T: fmt::Debug> fmt::Debug for Node<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("item", unsafe { self.item.as_ref() })
            .field("color", &self.color)
            .field("addr", &self.addr)
            .field("parent", &self.parent)
            .field("left", unsafe {
                if self.left.is_null() {
                    &"null"
                } else {
                    &*self.left
                }
            })
            .field("right", unsafe {
                if self.right.is_null() {
                    &"null"
                } else {
                    &*self.right
                }
            })
            .finish()
    }
}

impl<T: fmt::Debug> Node<T> {
    /// Create a new Node.
    ///
    /// # Safety
    /// The given `addr` must contain enough space for a Node<T> and be aligned.
    pub fn new(item: *mut T, addr: *mut Node<T>) -> Self {
        let node = Self {
            addr,
            item: unsafe { NonNull::new_unchecked(item) },
            color: NodeColor::Red,
            parent: ptr::null_mut(),
            left: ptr::null_mut(),
            right: ptr::null_mut(),
        };

        unsafe {
            ptr::write(addr, node);
            // I'm assuming this is safe since we just wrote it
            ptr::read(addr)
        }
    }

    pub fn left_id(&self) -> Option<usize> {
        if self.left.is_null() {
            None
        } else {
            unsafe { Some((*self.left).addr as usize) }
        }
    }

    pub fn right_id(&self) -> Option<usize> {
        if self.right.is_null() {
            None
        } else {
            unsafe { Some((*self.right).addr as usize) }
        }
    }

    pub fn parent(&self) -> *mut Node<T> {
        self.parent
    }

    pub fn grandparent(&self) -> *mut Node<T> {
        if !self.parent.is_null() {
            unsafe { (*self.parent).parent() }
        } else {
            ptr::null_mut()
        }
    }

    pub fn sibling(&self) -> *mut Node<T> {
        let p = self.parent();

        if p.is_null() {
            return p;
        }
        let p = unsafe { &*p };
        if Some(self.addr as usize) == p.left_id() {
            p.right
        } else {
            p.left
        }
    }

    /// A parents sibling.
    ///
    /// Siblings are found by going to the parent and comparing
    /// which child was the start and returning the other child.
    pub fn uncle(&self) -> *mut Node<T> {
        let p = self.parent();

        if p.is_null() {
            p
        } else {
            unsafe { (*p).sibling() }
        }
    }

    pub fn rotate_left(&mut self) {
        dbg!(&self);

        let new_node = self.right;
        let p = self.parent();
        // Cannot move an empty leaf to an internal node spot
        assert!(!new_node.is_null());

        unsafe {
            // shift the right child's left child up to right child
            self.right = (*new_node).left;
            // shift self down and out
            //          N                      N
            //       P     Q  ==>         nul      Q
            //    R    nul              R
            //                       P
            (*new_node).left = self.addr;
        }
        // Create parent link to complete above link
        self.parent = new_node;

        // If we just added a new right child set self as the parent
        if !self.right.is_null() {
            unsafe { (*self.right).parent = self.addr };
        }

        // Make sure this is not the root node
        if !p.is_null() {
            unsafe {
                // shift the new_node up to the correct parent "child"
                //           N                  N
                //      nul      Q ==>      R       Q
                //    R                 P
                // P
                if Some(self.addr as usize) == (*p).left_id() {
                    (*p).left = new_node;
                } else if Some(self.addr as usize) == (*p).right_id() {
                    (*p).right = new_node;
                }
            }
        }
        // Create the link back from the parent to complete above shifting
        unsafe {
            (*new_node).parent = p;
        }
    }

    pub fn rotate_right(&mut self) {
        let new_node = self.left;
        let p = self.parent();
        // Cannot move an empty leaf to an internal node spot
        assert!(!new_node.is_null());

        unsafe {
            self.left = (*new_node).right;
            (*new_node).right = self.addr;
        }
        self.parent = new_node;

        // If we just added a new left child set self as the parent
        if !self.left.is_null() {
            unsafe { (*self.left).parent = self.addr };
        }

        // If we are at the root
        if !p.is_null() {
            unsafe {
                if Some(self.addr as usize) == (*p).left_id() {
                    (*p).left = new_node;
                } else if Some(self.addr as usize) == (*p).right_id() {
                    (*p).right = new_node;
                }
            }
        }
        unsafe {
            (*new_node).parent = p;
        }
    }

    fn balance_insert(new: *mut Node<T>) {
        let node = if new.is_null() {
            panic!("new was null");
            return;
        } else {
            unsafe { &*new }
        };

        unsafe {
            if node.parent().is_null() {
                Self::balance_1(new);
            } else if (*node.parent()).color == NodeColor::Black {
                Self::balance_2(new);
            } else if !node.uncle().is_null() && (*node.uncle()).color == NodeColor::Red {
                Self::balance_3(new);
            } else {
                Self::balance_4(new);
            }
        }
    }

    fn balance_1(new: *mut Node<T>) {
        println!("balance 1");

        unsafe {
            (*new).color = NodeColor::Black;
        }
    }

    fn balance_2(new: *mut Node<T>) {
        /* No-OP tree is still valid */
        println!("balance 2");
    }

    fn balance_3(new: *mut Node<T>) {
        println!("balance 3");

        unsafe {
            (*(*new).parent()).color = NodeColor::Black;
            (*(*new).uncle()).color = NodeColor::Black;
            (*(*new).grandparent()).color = NodeColor::Red;
            Self::balance_insert((*new).grandparent())
        }
    }

    fn balance_4(mut new: *mut Node<T>) {
        println!("balance 4");
        unsafe {
            let p = (*new).parent();
            let g = (*new).grandparent();

            if new == (*p).right && p == (*g).left {
                (*p).rotate_left();
                new = (*new).left;
            } else if new == (*p).left && p == (*g).right {
                (*p).rotate_right();
                new = (*new).right;
            }

            // Our `new` may be different now
            let p = (*new).parent();
            let g = (*new).grandparent();

            if Some((*new).addr as usize) == (*p).left_id() {
                (*p).rotate_right()
            } else {
                (*p).rotate_left()
            }

            (*p).color = NodeColor::Black;
            (*g).color = NodeColor::Red;
        }
    }
}

impl<T: Ord + fmt::Debug> Node<T> {
    unsafe fn insert_recurs(root: *mut Node<T>, new: *mut Node<T>) -> *mut Node<T> {
        if new.item.as_ref() < self.item.as_ref() {
            if !self.left.is_null() {
                Self::insert_recurs(&mut (*self.left), new);
                return self.right;
            } else {
                self.left = new.addr;
            }
        } else if !self.right.is_null() {
            Self::insert_recurs(&mut (*self.right), new);
            return self.right;
        } else {
            self.right = new.addr;
        }

        new.parent = self.addr;
        new.left = ptr::null_mut();
        new.right = ptr::null_mut();
        new.color = NodeColor::Red;
        ptr::write(new.addr, new);
        ptr::write(self.addr, ptr::read(self));
        self.right
    }

    pub unsafe fn insert(root: *mut Node<T>, new: *mut Node<T>) {
        let new_ptr = Node::insert_recurs(root, new);

        Self::balance_insert(new_ptr);

        self.addr = new_ptr;
        while !self.parent().is_null() {
            self.addr = self.parent();
        }

        ptr::replace(self.addr, ptr::read(self.addr));
        dbg!(&self);
    }
}

pub struct Tree<T> {
    head: Node<T>,
}

impl<T> Tree<T> {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn create_tree() {
        let mut space = [0_u8; 10 * mem::size_of::<Node<u8>>()];
        let mut items = [0_u8; 10];
        let ptr = &mut space[0] as *mut u8 as *mut Node<u8>;
        let item_ptr = &mut items[0] as *mut u8;

        let mut root = ptr::null_mut();
        for idx in 0_u8..10 {
            unsafe {
                ptr::write(item_ptr.add(idx as usize), idx);
            }
            let mut node = unsafe { Node::new(item_ptr.add(idx as usize), ptr.add(idx as usize)) };
            if idx == 0 {
                node.color = NodeColor::Black;
                root = node.addr;
            } else {
                unsafe { Node::insert(root, node.addr) };
            }
        }

        println!("{:#?}", root)
    }
}
