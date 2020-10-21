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
                    &self.left
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

    pub unsafe fn left(node: *mut Node<T>) -> *mut Node<T> {
        if node.is_null() {
            ptr::null_mut()
        } else {
            (*node).left
        }
    }

    pub unsafe fn right(node: *mut Node<T>) -> *mut Node<T> {
        if node.is_null() {
            ptr::null_mut()
        } else {
            (*node).right
        }
    }

    pub unsafe fn left_id(node: *mut Node<T>) -> Option<usize> {
        if node.is_null() {
            return None;
        }
        if (*node).left.is_null() {
            None
        } else {
            unsafe { Some((*node).left as usize) }
        }
    }

    pub unsafe fn right_id(node: *mut Node<T>) -> Option<usize> {
        if node.is_null() {
            return None;
        }
        if (*node).right.is_null() {
            None
        } else {
            unsafe { Some((*node).right as usize) }
        }
    }

    pub unsafe fn parent(node: *mut Node<T>) -> *mut Node<T> {
        if node.is_null() {
            ptr::null_mut()
        } else {
            (*node).parent
        }
    }

    pub unsafe fn grandparent(node: *mut Node<T>) -> *mut Node<T> {
        if Node::parent(node).is_null() {
            ptr::null_mut()
        } else {
            Node::parent(Node::parent(node))
        }
    }

    pub unsafe fn sibling(node: *mut Node<T>) -> *mut Node<T> {
        let p = Node::parent(node);

        if p.is_null() {
            return p;
        }
        if Some(node as usize) == Node::left_id(p) {
            Node::right(p)
        } else {
            Node::left(p)
        }
    }

    /// A parents sibling.
    ///
    /// Siblings are found by going to the parent and comparing
    /// which child was the start and returning the other child.
    pub unsafe fn uncle(node: *mut Node<T>) -> *mut Node<T> {
        let p = Node::parent(node);

        if p.is_null() {
            ptr::null_mut()
        } else {
            Node::sibling(p)
        }
    }

    pub unsafe fn rotate_left(node: *mut Node<T>) {
        let new_node = Node::right(node);
        let p = Node::parent(node);
        // Cannot move an empty leaf to an internal node spot
        assert!(!new_node.is_null());

        // shift the right child's left child up to right child
        (*node).right = (*new_node).left;
        // shift self down and out
        //          N                      N
        //       P     Q  ==>         nul      Q
        //    R    nul              R
        //                       P
        (*new_node).left = node;
        // Create parent link to complete above link
        (*node).parent = new_node;

        // If we just added a new right child set self as the parent
        if !(*node).right.is_null() {
            (*(*node).right).parent = node;
        }

        // Make sure this is not the root node
        if !p.is_null() {
            unsafe {
                // shift the new_node up to the correct parent "child"
                //           N                  N
                //      nul      Q ==>      R       Q
                //    R                 P
                // P
                if node == Node::left(p) {
                    dbg!(&node);
                    dbg!(&(*node).addr);
                    (*p).left = new_node;
                } else if node == Node::right(p) {
                    (*p).right = new_node;
                }
            }
        }
        // Create the link back from the parent to complete above shifting
        unsafe {
            (*new_node).parent = p;
        }
    }

    pub unsafe fn rotate_right(node: *mut Node<T>) {
        let new_node = Node::left(node);
        let p = Node::parent(node);
        // Cannot move an empty leaf to an internal node spot
        assert!(!new_node.is_null());

        unsafe {
            (*node).left = Node::right(new_node);
            (*new_node).right = node;
        }
        (*node).parent = new_node;

        // If we just added a new left child set self as the parent
        if !(*node).left.is_null() {
            unsafe { (*(*node).left).parent = node };
        }

        // If we are at the root
        if !p.is_null() {
            unsafe {
                if node == Node::left(p) {
                    (*p).left = new_node;
                } else if node == Node::right(p) {
                    (*p).right = new_node;
                }
            }
        }
        unsafe {
            (*new_node).parent = p;
        }
    }

    fn balance_insert(new: *mut Node<T>) {
        if new.is_null() {
            panic!("new was null");
            return;
        };

        unsafe {
            if Node::parent(new).is_null() {
                Self::balance_1(new);
            } else if (*Node::parent(new)).color == NodeColor::Black {
                Self::balance_2(new);
            } else if !Node::uncle(new).is_null() && (*Node::uncle(new)).color == NodeColor::Red {
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
            (*Node::parent(new)).color = NodeColor::Black;
            (*Node::uncle(new)).color = NodeColor::Black;
            (*Node::grandparent(new)).color = NodeColor::Red;
            Self::balance_insert(Node::grandparent(new))
        }
    }

    unsafe fn balance_4(mut new: *mut Node<T>) {
        println!("balance 4");
        unsafe {
            let p = Node::parent(new);
            let g = Node::grandparent(new);

            dbg!(&*p);
            dbg!(&*new);

            if new == Node::right(p) && p == Node::left(g) {
                Node::rotate_left(p);
                new = (*new).left;
            } else if new == Node::left(p) && p == Node::right(g) {
                Node::rotate_right(p);
                new = (*new).right;
            }

            // Our `new` may be different now
            let p = Node::parent(new);
            let g = Node::grandparent(new);

            if new == Node::left(p) {
                Node::rotate_right(g);
            } else {
                Node::rotate_left(g);
            }

            if !p.is_null() {
                (*p).color = NodeColor::Black;
            }
            if !g.is_null() {
                (*g).color = NodeColor::Red;
            }
        }
    }
}

impl<T: Ord + fmt::Debug> Node<T> {
    unsafe fn insert_recurs(root: *mut Node<T>, new: *mut Node<T>) {
        if !root.is_null() {
            if (*new).item.as_ref() < (*root).item.as_ref() {
                if !(*root).left.is_null() {
                    Self::insert_recurs(Node::left(root), new);
                } else {
                    (*root).left = new;
                }
            } else if !(*root).right.is_null() {
                Self::insert_recurs(Node::right(root), new);
            } else {
                (*root).right = new;
            }
        }

        (*new).parent = root;
        (*new).left = ptr::null_mut();
        (*new).right = ptr::null_mut();
        (*new).color = NodeColor::Red;
    }

    pub unsafe fn insert(mut root: *mut Node<T>, new: *mut Node<T>) {
        Node::insert_recurs(root, new);
        Self::balance_insert(new);

        root = new;
        while !Node::parent(root).is_null() {
            root = Node::parent(root);
        }
        dbg!(root);
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
        let mut ptr = &mut space[0] as *mut u8 as *mut Node<u8>;
        let item_ptr = &mut items[0] as *mut u8;

        let mut root = ptr::null_mut();
        for idx in 0_u8..3 {
            unsafe {
                ptr::write(item_ptr.add(idx as usize), idx);
                // Node::new writes the data to the ptr
                // which is why we don't do the same as the above for the item_ptr
                let mut node = Node::new(item_ptr.add(idx as usize), ptr.add(idx as usize));
                if idx == 0 {
                    root = node.addr;
                    (*root).color = NodeColor::Black;
                    dbg!(&*root);
                } else {
                    Node::insert(root, node.addr);
                }
            }
        }

        unsafe {
            println!("hey");
            dbg!(&*root);
        }
    }
}
