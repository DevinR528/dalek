use core::{
    cell::UnsafeCell,
    cmp::{self, Ordering},
    fmt, mem,
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
    item: T,
    color: NodeColor,
    parent: *mut Node<T>,
    left: *mut Node<T>,
    right: *mut Node<T>,
}

impl<T: fmt::Debug> fmt::Debug for Node<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("item", &self.item)
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
    pub fn new(item: T, addr: *mut Node<T>) -> Self {
        let node = Self {
            addr,
            item,
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

    pub unsafe fn traverse_tree<F: Fn(&T, usize, &NodeColor)>(
        node: *mut Node<T>,
        call: &F,
        align: usize,
    ) {
        if node.is_null() {
            return;
        }
        Node::traverse_tree(Node::right(node), call, align + 5);
        call(&(*node).item, align, &(*node).color);
        Node::traverse_tree(Node::left(node), call, align + 5);
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
        if node == Node::left(p) {
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

    unsafe fn balance_1(new: *mut Node<T>) {
        (*new).color = NodeColor::Black;
    }

    fn balance_2(new: *mut Node<T>) {
        /* No-OP tree is still valid */
    }

    unsafe fn balance_3(new: *mut Node<T>) {
        (*Node::parent(new)).color = NodeColor::Black;
        (*Node::uncle(new)).color = NodeColor::Black;
        (*Node::grandparent(new)).color = NodeColor::Red;
        Self::balance_insert(Node::grandparent(new))
    }

    unsafe fn balance_4(mut new: *mut Node<T>) {
        let p = Node::parent(new);
        let g = Node::grandparent(new);

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

impl<T: Ord + Eq + fmt::Debug> Node<T> {
    unsafe fn insert_iter(mut root: *mut Node<T>, new: *mut Node<T>) {
        let mut current = root;
        while !current.is_null() {
            if (*new).item < (*current).item {
                if !(*current).left.is_null() {
                    current = Node::left(current);
                } else {
                    (*current).left = new;
                    break;
                }
            } else if !(*current).right.is_null() {
                current = Node::right(current);
            } else {
                (*current).right = new;
                break;
            }
        }

        (*new).parent = current;
        (*new).left = ptr::null_mut();
        (*new).right = ptr::null_mut();
        (*new).color = NodeColor::Red;
    }

    /// Insert `new` in the first leaf to the right if `T` is larger, to the
    /// left if `T` is smaller.
    ///
    /// Returns the value that should be used as the root.
    ///
    /// # Safety
    /// `new` must always be a valid non-null pointer.
    /// `root` can be null, when inserting the first `Node` into the root (a one element tree)
    /// this will succeed.
    unsafe fn insert(mut root: *mut Node<T>, new: *mut Node<T>) -> *mut Node<T> {
        Node::insert_iter(root, new);
        Self::balance_insert(new);

        root = new;
        while !Node::parent(root).is_null() {
            root = Node::parent(root);
        }
        root
    }

    unsafe fn delete_case1(node: *mut Node<T>) {
        if !Node::parent(node).is_null() {
            Node::delete_case2(node);
        }
    }

    unsafe fn delete_case2(node: *mut Node<T>) {
        let sib = Node::sibling(node);

        if !sib.is_null() && (*sib).color == NodeColor::Red {
            (*(*node).parent).color = NodeColor::Red;
            (*sib).color = NodeColor::Black;
            if node == Node::left(Node::parent(node)) {
                Node::rotate_left((*node).parent);
            } else {
                Node::rotate_right((*node).parent);
            }
        }

        Node::delete_case3(node)
    }

    unsafe fn delete_case3(node: *mut Node<T>) {
        let sib = Node::sibling(node);

        if !sib.is_null() && (*sib).color == NodeColor::Red {
            (*(*node).parent).color = NodeColor::Red;
            (*sib).color = NodeColor::Black;
            if node == Node::left(Node::parent(node)) {
                Node::rotate_left((*node).parent);
            } else {
                Node::rotate_right((*node).parent);
            }
        }

        Node::delete_case4(node)
    }

    unsafe fn delete_case4(node: *mut Node<T>) {}

    /// Detach the `Node` and replace it with `child`.
    ///
    /// # Safety
    /// Both pointers must be __non null__.
    unsafe fn replace_node(node: *mut Node<T>, child: *mut Node<T>) {
        (*child).parent = (*node).parent;

        // is this the left node?
        if node == Node::left(Node::parent(node)) {
            (*(*node).parent).left = child;
        } else {
            (*(*node).parent).right = child;
        }
    }

    unsafe fn delete_node(node: *mut Node<T>) {
        let child = if Node::right(node).is_null() {
            Node::left(node)
        } else {
            Node::right(node)
        };
        assert!(!child.is_null());

        Node::replace_node(node, child);
        if (*node).color == NodeColor::Black {
            if (*child).color == NodeColor::Red {
                (*child).color = NodeColor::Black;
            } else {
                Node::delete_case1(child);
            }
        }
    }

    /// Unhook the `Node` that matches `key` form the tree.
    ///
    /// # Safety
    ///
    /// We return the the pointer to the `Node<T>` that matches `key`. This operation
    /// does __NOT__ free the memory the `Node<T>` occupies, the `Tree<T>` must
    /// keep track of it. The pointer returned can be null.
    unsafe fn delete(mut root: *mut Node<T>, key: &T) -> *mut Node<T> {
        let mut current = root;
        while !current.is_null() {
            match (*current).item.cmp(key) {
                Ordering::Greater if !(*current).right.is_null() => {
                    current = Node::right(current);
                }
                Ordering::Less if !(*current).left.is_null() => {
                    current = Node::left(current);
                }
                _ => break,
            }
        }

        Node::delete_node(current);
        current
    }
}

pub struct Tree<T> {
    /// The root `Node<T>` of our `Tree<T>`.
    head: *mut Node<T>,
    /// The chunk of memory used to store the `Node<T>`'s.
    backing: *mut Node<T>,
    /// What our current offset is into `backing`.
    idx: usize,
    /// The capacity of our backing memory.
    cap: usize,
    /// The current number of `Node<T>`s in backing.
    len: usize,
}

impl<T> Tree<T> {
    pub fn new(backing: *mut Node<T>, cap: usize) -> Self {
        Self {
            head: ptr::null_mut(),
            backing,
            idx: 0,
            cap,
            len: 0,
        }
    }
}

impl<T: Ord + fmt::Debug> Tree<T> {
    pub fn insert(&mut self, item: T) {
        if self.len < self.cap {
            unsafe {
                let next = Node::new(item, self.backing.add(self.idx));
                self.idx += 1;
                // `self.head` can be null here, it will be set.
                self.head = Node::insert(self.head, next.addr);
            }
        } else {
            panic!("not enough capacity: cap {}, len {}", self.cap, self.len)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn print_color<T: Ord + fmt::Display>(val: &T, align: usize, color: &NodeColor) {
        println!(
            "{}{}{}\x1B[0m",
            " ".repeat(align),
            match color {
                NodeColor::Red => "\x1B[31m",   // Red
                NodeColor::Black => "\x1B[32m", // Green
            },
            format!("({}{})", "0".repeat(2 - val.to_string().len()), val)
        )
    }

    #[test]
    fn node_insert() {
        let mut space = [0_u8; 20 * mem::size_of::<Node<u8>>()];
        let mut items = [10_u8, 9, 11, 8, 5, 15, 16, 20, 1, 2, 3, 23, 6];

        let mut ptr = &mut space[0] as *mut u8 as *mut Node<u8>;

        let mut root = ptr::null_mut();
        for idx in 0_u8..13 {
            unsafe {
                // Node::new writes the data to the ptr
                // which is why we don't do the same as the above for the item_ptr
                let mut node = Node::new(items[idx as usize], ptr.add(idx as usize));
                if idx == 0 {
                    root = node.addr;
                    (*root).color = NodeColor::Black;
                } else {
                    root = Node::insert(root, node.addr);
                }
            }
        }

        unsafe {
            dbg!(&&&*root);
            Node::traverse_tree(root, &print_color, 0);
        }
    }

    #[test]
    fn node_remove() {
        let mut space = [0_u8; 20 * mem::size_of::<Node<u8>>()];
        let mut items = [10_u8, 9, 11, 8, 5, 15, 16, 20, 1, 2, 3, 23, 6];

        let mut items = [4u8, 5, 3, 6, 2, 7, 1, 8];
        let mut ptr = &mut space[0] as *mut u8 as *mut Node<u8>;

        let mut root = ptr::null_mut();
        for idx in 0_u8..7 {
            unsafe {
                // Node::new writes the data to the ptr
                // which is why we don't do the same as the above for the item_ptr
                let mut node = Node::new(items[idx as usize], ptr.add(idx as usize));
                if idx == 0 {
                    root = node.addr;
                    (*root).color = NodeColor::Black;
                } else {
                    root = Node::insert(root, node.addr);
                }
            }
        }

        unsafe {
            //        4
            //      /   \
            //     2     6
            //    / \   / \
            //  (1)  3  5  7
            Node::delete(root, &1);

            dbg!(&&&*root);
            Node::traverse_tree(root, &print_color, 0);
        }
    }

    #[test]
    fn create_tree() {
        let mut space = [0_u8; 20 * mem::size_of::<Node<u8>>()];

        let mut items = [4u8, 5, 3, 6, 2, 7, 1, 8];
        let mut ptr = &mut space[0] as *mut u8 as *mut Node<u8>;

        let mut root = Tree::new(ptr, 20);
        for idx in 0_u8..7 {
            root.insert(items[idx as usize]);
        }

        unsafe {
            Node::traverse_tree(root.head, &print_color, 0);
        }
    }
}
