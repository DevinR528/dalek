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

static mut SEED: u64 = 0x9E3779B97F4A7C15;

/// Returns psudo-random number and modifies `SEED`.
///
/// Uses primes but is in no way secure it's not even truly random.
pub fn split_mix_64() -> usize {
    let mut z = unsafe {
        SEED = SEED.wrapping_add(0x9E3779B97F4A7C15);
        SEED
    };
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);

    (z ^ (z >> 31)) as usize
}

pub fn is_even(x: usize) -> bool {
    x % 2 == 0
}

/// Returns a semi random bool to determine which child
/// to start a removal from.
///
/// When a `Node<T>` is found for removal one can either choose
/// the left child's most right descendant or the right child's most
/// left descendant.
fn start_right() -> bool {
    is_even(split_mix_64())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
            .field("address", &self.addr)
            .field("parent ", &self.parent)
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

    /// Search for the smallest child (most left).
    ///
    /// # Safety
    pub unsafe fn most_left(node: *mut Node<T>) -> *mut Node<T> {
        let mut left = node;
        while !Node::left(left).is_null() {
            left = Node::left(left);
        }
        left
    }

    /// Search for the largest child (most right).
    ///
    /// # Safety
    pub unsafe fn most_right(node: *mut Node<T>) -> *mut Node<T> {
        let mut right = node;
        while !Node::right(right).is_null() {
            right = Node::right(right);
        }
        right
    }

    /// Compares `node`s color to the given `color`.
    ///
    /// # Safety
    /// If the pointer is null this will return false.
    // TODO this may not be a good idea
    pub unsafe fn cmp_color(node: *mut Node<T>, color: NodeColor) -> bool {
        if node.is_null() {
            false
        } else {
            (*node).color == color
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
                    // dbg!(&node);
                    // dbg!(&(*node).addr);
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

    unsafe fn delete_case1(node: *mut Node<T>) {
        println!("case 1");
        if !Node::parent(node).is_null() {
            Node::delete_case2(node);
        }
    }

    unsafe fn delete_case2(node: *mut Node<T>) {
        println!("case 2");

        let sib = Node::sibling(node);

        if Node::cmp_color(sib, NodeColor::Red) {
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
        println!("case 3");

        let sib = Node::sibling(node);

        if Node::cmp_color(Node::parent(node), NodeColor::Black)
            && Node::cmp_color(sib, NodeColor::Black)
            && Node::cmp_color(Node::left(sib), NodeColor::Black)
            && Node::cmp_color(Node::right(sib), NodeColor::Black)
        {
            (*sib).color = NodeColor::Red;
            Node::delete_case1(Node::parent(node));
        } else {
            Node::delete_case4(node);
        }
    }

    unsafe fn delete_case4(node: *mut Node<T>) {
        println!("case 4");

        let sib = Node::sibling(node);

        if Node::cmp_color(Node::parent(node), NodeColor::Red)
            && Node::cmp_color(sib, NodeColor::Black)
            && Node::cmp_color(Node::left(sib), NodeColor::Black)
            && Node::cmp_color(Node::right(sib), NodeColor::Black)
        {
            (*sib).color = NodeColor::Red;
            (*(*node).parent).color = NodeColor::Black;
        } else {
            Node::delete_case5(node);
        }
    }

    unsafe fn delete_case5(node: *mut Node<T>) {
        println!("case 5");

        let sib = Node::sibling(node);

        if Node::cmp_color(sib, NodeColor::Black) {
            // Force the red on to the left of the left parent or
            // right of the right parent
            if node == Node::left(Node::parent(node))
                && Node::cmp_color(Node::right(sib), NodeColor::Black)
                && Node::cmp_color(Node::left(sib), NodeColor::Red)
            {
            } else if node == Node::right(Node::parent(node))
                && Node::cmp_color(Node::left(sib), NodeColor::Black)
                && Node::cmp_color(Node::right(sib), NodeColor::Red)
            {
                (*sib).color = NodeColor::Red;
                (*(*sib).right).color = NodeColor::Black;
                Node::rotate_left(sib);
            }
        }

        Node::delete_case6(node);
    }

    unsafe fn delete_case6(node: *mut Node<T>) {
        println!("case 6");

        let sib = Node::sibling(node);

        (*sib).color = (*Node::parent(node)).color;
        (*(*node).parent).color = NodeColor::Black;

        if node == Node::left(Node::parent(node)) {
            (*(*sib).right).color = NodeColor::Black;
            Node::rotate_left(Node::parent(node));
        } else {
            (*(*sib).left).color = NodeColor::Black;
            Node::rotate_right(Node::parent(node));
        }
    }

    /// Binary Search Tree delete.
    ///
    /// # Safety
    /// It ain't yet.
    unsafe fn bst_delete(node: *mut Node<T>) {
        // TODO: I think we need to alternate between left sub
        // tree and right subtree smallest node in right
        // largest node in left
        //
        // TODO: we are now alternating see how that goes
        let predecessor = Node::most_left(Node::right(node));
        let successor = Node::most_right(Node::left(node));

        let most = if start_right() {
            successor
        } else {
            predecessor
        };

        // swap values, our left/right most value is now node.item and recurse
        mem::swap(&mut (*node).item, &mut (*most).item);
        Node::delete_node(most);
    }

    /// Detach the `Node` and replace it with `child`.
    ///
    /// # Safety
    /// Both pointers must be __non null__.
    unsafe fn replace_node(node: *mut Node<T>, child: *mut Node<T>) {
        //        4                     4
        //      /   \                 /   \
        //    (2)    6      ==>      1     6
        //    / \   / \               \   /  \
        //   1  3  5   7               3  5   7
        (*child).parent = (*node).parent;

        // is this the left node?
        if node == Node::left(Node::parent(node)) {
            (*(*node).parent).left = child;
        } else {
            (*(*node).parent).right = child;
        }
        // dbg!(&*node);
    }

    // TODO have a callback when the node is detached to deal with the memory?
    unsafe fn delete_node(node: *mut Node<T>) {
        // Both children are present we go to either the
        // right child's left until we find a null left Node or the left child's
        // right.
        if !Node::right(node).is_null() && !Node::left(node).is_null() {
            Node::bst_delete(node);
            return;
        }

        let child = if Node::right(node).is_null() {
            Node::left(node)
        } else {
            Node::right(node)
        };
        // Special case to handle deleting a red `Node` with no children
        if child.is_null() && Node::cmp_color(node, NodeColor::Red) {
            if node == Node::left(Node::parent(node)) {
                (*(*node).parent).left = ptr::null_mut();
                return;
            } else if node == Node::right(Node::parent(node)) {
                (*(*node).parent).right = ptr::null_mut();
                return;
            }
        } else if child.is_null() && Node::cmp_color(node, NodeColor::Black) {
            // The much more complicated case of a black node
            panic!()
        }

        Node::replace_node(node, child);
        if (*node).color == NodeColor::Black {
            if (*child).color == NodeColor::Red {
                (*child).color = NodeColor::Black;
            } else {
                Node::delete_case1(child);
            }
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

    /// Unhook the `Node` that matches `key` form the tree.
    ///
    /// # Safety
    ///
    /// We return the the pointer to the `Node<T>` that matches `key`. This operation
    /// does __NOT__ free the memory the `Node<T>` occupies, the `Tree<T>` must
    /// keep track of it. The pointer returned can be null if `root` is null.
    unsafe fn delete(mut root: *mut Node<T>, key: &T) -> Option<*mut Node<T>> {
        let mut current = root;
        while !current.is_null() {
            match (*current).item.cmp(key) {
                Ordering::Less if !(*current).right.is_null() => {
                    current = Node::right(current);
                }
                Ordering::Greater if !(*current).left.is_null() => {
                    current = Node::left(current);
                }
                Ordering::Equal => break,
                _ => return None,
            }
        }

        Node::delete_node(current);
        Some(current)
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

    pub fn remove(&mut self, key: &T) -> Option<T> {
        Some(unsafe {
            let ptr = Node::delete(self.head, key)?;
            // TODO: keep track of this freed Node<T> for reallocation/free

            if ptr.is_null() {
                panic!()
            } else {
                ptr::read(ptr).item
            }
        })
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
    fn rnd_test() {
        let mut v = vec![];
        for x in 0..30 {
            let y = split_mix_64();
            v.push(is_even(y));
        }
        // println!("{:?}", v);
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
            // Node::traverse_tree(root, &print_color, 0);
        }
    }

    #[test]
    fn node_remove1() {
        let mut space = [0_u8; 20 * mem::size_of::<Node<u8>>()];
        let mut items = [4u8, 5, 3, 6, 2, 7, 1];
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
            //  (1)  3  5  (7)
            Node::delete(root, &1);
            Node::delete(root, &7);

            // Node::traverse_tree(root, &print_color, 0);
        }
    }

    #[test]
    fn node_remove2() {
        let mut space = [0_u8; 20 * mem::size_of::<Node<u8>>()];
        let mut items = [4u8, 5, 3, 6, 2, 7, 1];
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
            // The following diagrams are correct if and only if the deletion algorithm
            // has a left leaning bst delete. If we use a combo of right child -> left and
            // left child -> right a more balanced tree is maintained.
            //        4
            //      /   \
            //    (2)    6
            //    / \   / \
            //   1  3  5   7
            Node::delete(root, &2);
            //        4
            //      /   \
            //     1    (6)
            //      \   / \
            //      3  5   7
            Node::delete(root, &6);
            //        4
            //      /   \
            //     1     5
            //      \     \
            //      3      7

            // Node::traverse_tree(root, &print_color, 0);
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
            // Node::traverse_tree(root.head, &print_color, 0);
        }
    }
}
