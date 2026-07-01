// https://man.openbsd.org/tree.3
// probably best way define a generic struct
// make the macros call the generic struct
use ::core::cmp::Ordering;
use ::core::ptr::{NonNull, null_mut};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct rb_head<T> {
    pub rbh_root: *mut T,
}

impl<T> Default for rb_head<T> {
    fn default() -> Self {
        Self {
            rbh_root: null_mut(),
        }
    }
}

impl<T> rb_head<T> {
    pub(crate) const fn rb_init() -> Self {
        Self {
            rbh_root: null_mut(),
        }
    }
}

#[repr(i32)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum rb_color {
    #[default]
    RB_BLACK = 0,
    RB_RED = 1,
}

#[repr(C)]
pub struct rb_entry<T> {
    pub rbe_left: *mut T,
    pub rbe_right: *mut T,
    pub rbe_parent: *mut T,
    rbe_color: rb_color,
}

impl<T> std::fmt::Debug for rb_entry<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("rb_entry")
            .field("rbe_left", &self.rbe_left)
            .field("rbe_right", &self.rbe_right)
            .field("rbe_parent", &self.rbe_parent)
            .field("rbe_color", &self.rbe_color)
            .finish()
    }
}

impl<T> Default for rb_entry<T> {
    fn default() -> Self {
        Self {
            rbe_left: null_mut(),
            rbe_right: null_mut(),
            rbe_parent: null_mut(),
            rbe_color: rb_color::default(),
        }
    }
}

impl<T> Copy for rb_entry<T> {}
impl<T> Clone for rb_entry<T> {
    fn clone(&self) -> Self {
        *self
    }
}

pub trait GetEntry<T, D = ()> {
    unsafe fn entry_mut(this: *mut Self) -> *mut rb_entry<T>;
    unsafe fn entry_const(this: *const Self) -> *const rb_entry<T>;
    fn cmp(this: &Self, other: &Self) -> std::cmp::Ordering;
}

pub const unsafe fn rb_init<T>(head: *mut rb_head<T>) {
    unsafe {
        (*head).rbh_root = null_mut();
    }
}

pub const fn rb_initializer<T>() -> rb_head<T> {
    rb_head {
        rbh_root: null_mut(),
    }
}

unsafe fn rb_left<T, D>(this: *mut T) -> *mut T
where
    T: GetEntry<T, D>,
{
    unsafe { (*T::entry_mut(this)).rbe_left }
}

unsafe fn rb_left_const<T, D>(this: *const T) -> *const T
where
    T: GetEntry<T, D>,
{
    unsafe { (*T::entry_const(this)).rbe_left }
}

unsafe fn rb_set_left<T, D>(this: *mut T, value: *mut T)
where
    T: GetEntry<T, D>,
{
    unsafe {
        (*T::entry_mut(this)).rbe_left = value;
    }
}

#[inline]
unsafe fn is_left_sibling<T, D>(this: *const T) -> bool
where
    T: GetEntry<T, D>,
{
    unsafe { this == rb_left_const(rb_parent_const(this)) }
}

#[inline]
unsafe fn is_right_sibling<T, D>(this: *const T) -> bool
where
    T: GetEntry<T, D>,
{
    unsafe { this == rb_right_const(rb_parent_const(this)) }
}

unsafe fn rb_right<T, D>(this: *mut T) -> *mut T
where
    T: GetEntry<T, D>,
{
    unsafe { (*T::entry_mut(this)).rbe_right }
}

unsafe fn rb_set_right<T, D>(this: *mut T, value: *mut T)
where
    T: GetEntry<T, D>,
{
    unsafe {
        (*T::entry_mut(this)).rbe_right = value;
    }
}

unsafe fn rb_right_const<T, D>(this: *const T) -> *const T
where
    T: GetEntry<T, D>,
{
    unsafe { (*T::entry_const(this)).rbe_right }
}

unsafe fn rb_parent<T, D>(this: *mut T) -> *mut T
where
    T: GetEntry<T, D>,
{
    unsafe { (*T::entry_mut(this)).rbe_parent }
}

unsafe fn rb_set_parent<T, D>(this: *mut T, value: *mut T)
where
    T: GetEntry<T, D>,
{
    unsafe {
        (*T::entry_mut(this)).rbe_parent = value;
    }
}

unsafe fn rb_parent_const<T, D>(this: *const T) -> *const T
where
    T: GetEntry<T, D>,
{
    unsafe { (*T::entry_const(this)).rbe_parent }
}

unsafe fn rb_color<T, D>(elm: *mut T) -> rb_color
where
    T: GetEntry<T, D>,
{
    unsafe { (*T::entry_mut(elm)).rbe_color }
}

unsafe fn rb_set_color<T, D>(elm: *mut T, color: rb_color)
where
    T: GetEntry<T, D>,
{
    unsafe { (*T::entry_mut(elm)).rbe_color = color }
}

pub unsafe fn rb_root<T>(head: *mut rb_head<T>) -> *mut T {
    unsafe { (*head).rbh_root }
}

unsafe fn rb_set_root<T>(head: *mut rb_head<T>, value: *mut T) {
    unsafe { (*head).rbh_root = value }
}

pub unsafe fn rb_empty<T>(head: *const rb_head<T>) -> bool {
    unsafe { (*head).rbh_root.is_null() }
}
unsafe fn rb_set<T, D>(elm: *mut T, parent: *mut T)
where
    T: GetEntry<T, D>,
{
    unsafe {
        let ptr = T::entry_mut(elm);
        (*ptr).rbe_parent = parent;
        (*ptr).rbe_right = null_mut();
        (*ptr).rbe_left = null_mut();
        (*ptr).rbe_color = rb_color::RB_RED;
    }
}

unsafe fn rb_set_blackred<T, D>(black: *mut T, red: *mut T)
where
    T: GetEntry<T, D>,
{
    unsafe {
        (*T::entry_mut(black)).rbe_color = rb_color::RB_BLACK;
        (*T::entry_mut(red)).rbe_color = rb_color::RB_RED;
    }
}

unsafe fn rb_rotate_left<T, D>(head: *mut rb_head<T>, elm: *mut T)
where
    T: GetEntry<T, D>,
{
    unsafe {
        let tmp = rb_right(elm);
        rb_set_right(elm, rb_left(tmp));
        if !rb_right(elm).is_null() {
            rb_set_parent(rb_left(tmp), elm);
        }
        rb_set_parent(tmp, rb_parent(elm));
        if !rb_parent(tmp).is_null() {
            if is_left_sibling(elm) {
                rb_set_left(rb_parent(elm), tmp);
            } else {
                rb_set_right(rb_parent(elm), tmp);
            }
        } else {
            (*head).rbh_root = tmp;
        }

        rb_set_left(tmp, elm);
        rb_set_parent(elm, tmp);
    }
}

unsafe fn rb_rotate_right<T, D>(head: *mut rb_head<T>, elm: *mut T)
where
    T: GetEntry<T, D>,
{
    unsafe {
        let tmp = rb_left(elm);
        rb_set_left(elm, rb_right(tmp));
        if !rb_left(elm).is_null() {
            rb_set_parent(rb_right(tmp), elm);
        }
        rb_set_parent(tmp, rb_parent(elm));
        if !rb_parent(tmp).is_null() {
            if is_left_sibling(elm) {
                rb_set_left(rb_parent(elm), tmp);
            } else {
                rb_set_right(rb_parent(elm), tmp);
            }
        } else {
            (*head).rbh_root = tmp;
        }
        rb_set_right(tmp, elm);
        rb_set_parent(elm, tmp);
    }
}

// RB_GENERATE_STATIC name, type, field, cmp
macro_rules! RB_GENERATE {
    ($head_ty:ty, $ty:ty, $entry_field:ident, $entry_field_discr:ty, $cmp_fn:ident) => {
        impl $crate::compat::tree::GetEntry<$ty, $entry_field_discr> for $ty {
            unsafe fn entry_const(this: *const Self) -> *const rb_entry<$ty> {
                unsafe { &raw const (*this).$entry_field }
            }
            unsafe fn entry_mut(this: *mut Self) -> *mut rb_entry<$ty> {
                unsafe { &raw mut (*this).$entry_field }
            }
            fn cmp(this: &Self, other: &Self) -> std::cmp::Ordering {
                $cmp_fn(this, other)
            }
        }
    };
}
pub(crate) use RB_GENERATE;

unsafe fn rb_minmax_const<T, D>(head: *const rb_head<T>, val: i32) -> *const T
where
    T: GetEntry<T, D>,
{
    unsafe {
        let mut tmp: *const T = (*head).rbh_root;
        let mut parent: *const T = null_mut();

        while !tmp.is_null() {
            parent = tmp;
            if val < 0 {
                tmp = rb_left_const(tmp);
            } else {
                tmp = rb_right_const(tmp);
            }
        }

        parent
    }
}

unsafe fn rb_minmax<T, D>(head: *mut rb_head<T>, val: i32) -> *mut T
where
    T: GetEntry<T, D>,
{
    unsafe {
        let mut tmp: *mut T = (*head).rbh_root;
        let mut parent: *mut T = null_mut();

        while !tmp.is_null() {
            parent = tmp;
            if val < 0 {
                tmp = rb_left(tmp);
            } else {
                tmp = rb_right(tmp);
            }
        }

        parent
    }
}

pub unsafe fn rb_insert_color<T, D>(head: *mut rb_head<T>, mut elm: *mut T)
where
    T: GetEntry<T, D>,
{
    unsafe {
        while let Some(parent) = NonNull::new(rb_parent(elm))
            && rb_color(parent.as_ptr()) == rb_color::RB_RED
        {
            #[expect(clippy::shadow_reuse)]
            let mut parent = parent.as_ptr();
            let gparent = rb_parent(parent);
            if parent == rb_left(gparent) {
                let mut tmp = rb_right(gparent);
                if !tmp.is_null() && rb_color(tmp) == rb_color::RB_RED {
                    rb_set_color(tmp, rb_color::RB_BLACK);
                    rb_set_blackred(parent, gparent);
                    elm = gparent;
                    continue;
                }
                if rb_right(parent) == elm {
                    rb_rotate_left(head, parent);
                    tmp = parent;
                    parent = elm;
                    elm = tmp;
                }
                rb_set_blackred(parent, gparent);
                rb_rotate_right(head, gparent);
            } else {
                let mut tmp = rb_left(gparent);
                if !tmp.is_null() && rb_color(tmp) == rb_color::RB_RED {
                    rb_set_color(tmp, rb_color::RB_BLACK);
                    rb_set_blackred(parent, gparent);
                    elm = gparent;
                    continue;
                }
                if rb_left(parent) == elm {
                    rb_rotate_right(head, parent);
                    tmp = parent;
                    parent = elm;
                    elm = tmp;
                }
                rb_set_blackred(parent, gparent);
                rb_rotate_left(head, gparent);
            }
        }
        (*T::entry_mut((*head).rbh_root)).rbe_color = rb_color::RB_BLACK;
    }
}

unsafe fn rb_remove_color<T, D>(head: *mut rb_head<T>, mut parent: *mut T, mut elm: *mut T)
where
    T: GetEntry<T, D>,
{
    unsafe {
        let mut tmp: *mut T;
        while (elm.is_null() || rb_color(elm) == rb_color::RB_BLACK) && elm != rb_root(head) {
            if rb_left(parent) == elm {
                tmp = rb_right(parent);

                if rb_color(tmp) == rb_color::RB_RED {
                    rb_set_blackred(tmp, parent);
                    rb_rotate_left(head, parent);
                    tmp = rb_right(parent);
                }
                if (rb_left(tmp).is_null() || rb_color(rb_left(tmp)) == rb_color::RB_BLACK)
                    && (rb_right(tmp).is_null() || rb_color(rb_right(tmp)) == rb_color::RB_BLACK)
                {
                    rb_set_color(tmp, rb_color::RB_RED);
                    elm = parent;
                    parent = rb_parent(elm);
                } else {
                    if rb_right(tmp).is_null() || rb_color(rb_right(tmp)) == rb_color::RB_BLACK {
                        let oleft = rb_left(tmp);
                        if !oleft.is_null() {
                            rb_set_color(oleft, rb_color::RB_BLACK);
                        }
                        rb_set_color(tmp, rb_color::RB_RED);
                        rb_rotate_right(head, tmp);
                        tmp = rb_right(parent);
                    }
                    rb_set_color(tmp, rb_color(parent));
                    rb_set_color(parent, rb_color::RB_BLACK);
                    if !rb_right(tmp).is_null() {
                        rb_set_color(rb_right(tmp), rb_color::RB_BLACK);
                    }
                    rb_rotate_left(head, parent);
                    elm = rb_root(head);
                    break;
                }
            } else {
                tmp = rb_left(parent);
                if rb_color(tmp) == rb_color::RB_RED {
                    rb_set_blackred(tmp, parent);
                    rb_rotate_right(head, parent);
                    tmp = rb_left(parent);
                }
                if (rb_left(tmp).is_null() || rb_color(rb_left(tmp)) == rb_color::RB_BLACK)
                    && (rb_right(tmp).is_null() || rb_color(rb_right(tmp)) == rb_color::RB_BLACK)
                {
                    rb_set_color(tmp, rb_color::RB_RED);
                    elm = parent;
                    parent = rb_parent(elm);
                } else {
                    if rb_left(tmp).is_null() || rb_color(rb_left(tmp)) == rb_color::RB_BLACK {
                        let oright = rb_right(tmp);
                        if !oright.is_null() {
                            rb_set_color(oright, rb_color::RB_BLACK);
                        }
                        rb_set_color(tmp, rb_color::RB_RED);
                        rb_rotate_left(head, tmp);
                        tmp = rb_left(parent);
                    }
                    rb_set_color(tmp, rb_color(parent));
                    rb_set_color(parent, rb_color::RB_BLACK);
                    if !rb_left(tmp).is_null() {
                        rb_set_color(rb_left(tmp), rb_color::RB_BLACK);
                    }
                    rb_rotate_right(head, parent);
                    elm = rb_root(head);
                    break;
                }
            }
        }

        if !elm.is_null() {
            rb_set_color(elm, rb_color::RB_BLACK);
        }
    }
}

pub unsafe fn rb_remove<T, D>(head: *mut rb_head<T>, mut elm: *mut T) -> *mut T
where
    T: GetEntry<T, D>,
{
    unsafe {
        let old: *mut T = elm;
        let child: *mut T;
        let mut parent: *mut T;
        let color: rb_color;

        'color: {
            if rb_left(elm).is_null() {
                child = rb_right(elm);
            } else if rb_right(elm).is_null() {
                child = rb_left(elm);
            } else {
                elm = rb_right(elm);
                let mut left: *mut T;
                while {
                    left = rb_left(elm);
                    !left.is_null()
                } {
                    elm = left;
                }
                child = rb_right(elm);
                parent = rb_parent(elm);
                color = rb_color(elm);
                if !child.is_null() {
                    rb_set_parent(child, parent);
                }
                if !parent.is_null() {
                    if rb_left(parent) == elm {
                        rb_set_left(parent, child);
                    } else {
                        rb_set_right(parent, child);
                    }
                } else {
                    rb_set_root(head, child);
                }
                if rb_parent(elm) == old {
                    parent = elm;
                }
                *GetEntry::entry_mut(elm) = *GetEntry::entry_mut(old);
                if !rb_parent(old).is_null() {
                    if is_left_sibling(old) {
                        rb_set_left(rb_parent(old), elm);
                    } else {
                        rb_set_right(rb_parent(old), elm);
                    }
                } else {
                    rb_set_root(head, elm);
                }
                rb_set_parent(rb_left(old), elm);
                if !rb_right(old).is_null() {
                    rb_set_parent(rb_right(old), elm);
                }
                if !parent.is_null() {
                    left = parent;

                    while {
                        left = rb_parent(left);
                        !left.is_null()
                    } {}
                }
                break 'color;
            }

            parent = rb_parent(elm);
            color = rb_color(elm);
            if !child.is_null() {
                rb_set_parent(child, parent);
            }
            if !parent.is_null() {
                if rb_left(parent) == elm {
                    rb_set_left(parent, child);
                } else {
                    rb_set_right(parent, child);
                }
            } else {
                rb_set_root(head, child);
            }
        }
        // color:
        if color == rb_color::RB_BLACK {
            rb_remove_color(head, parent, child);
        }
        old
    }
}

pub unsafe fn rb_insert<T, D>(head: *mut rb_head<T>, elm: *mut T) -> *mut T
where
    T: GetEntry<T, D>,
{
    unsafe {
        let mut parent = null_mut();
        let mut comp = Ordering::Equal;

        let mut tmp = rb_root(head);
        while !tmp.is_null() {
            parent = tmp;

            comp = T::cmp(&*elm, &*parent);
            tmp = match comp {
                Ordering::Less => rb_left(tmp),
                Ordering::Greater => rb_right(tmp),
                Ordering::Equal => return tmp,
            };
        }
        rb_set(elm, parent);
        if !parent.is_null() {
            if matches!(comp, Ordering::Less) {
                rb_set_left(parent, elm);
            } else {
                rb_set_right(parent, elm);
            }
        } else {
            rb_set_root(head, elm);
        }
        rb_insert_color(head, elm);
    }
    null_mut()
}

// note the ordering from this must be the same as the default comparator
pub unsafe fn rb_find_by<T, D, F>(head: *mut rb_head<T>, cmp: F) -> *mut T
where
    T: GetEntry<T, D>,
    F: Fn(&T) -> std::cmp::Ordering,
{
    unsafe {
        let mut tmp: *mut T = (*head).rbh_root;

        while !tmp.is_null() {
            tmp = match cmp(&*tmp) {
                Ordering::Less => rb_left(tmp),
                Ordering::Greater => rb_right(tmp),
                Ordering::Equal => return tmp,
            };
        }
    }

    null_mut()
}

pub unsafe fn rb_find_by_const<T, D, F>(head: &rb_head<T>, cmp: F) -> *const T
where
    T: GetEntry<T, D>,
    F: Fn(&T) -> std::cmp::Ordering,
{
    unsafe {
        let mut tmp: *const T = head.rbh_root.cast_const();

        while !tmp.is_null() {
            tmp = match cmp(&*tmp) {
                Ordering::Less => rb_left_const(tmp),
                Ordering::Greater => rb_right_const(tmp),
                Ordering::Equal => return tmp,
            };
        }
    }

    null_mut()
}

pub unsafe fn rb_find<T, D>(head: *mut rb_head<T>, elm: *const T) -> *mut T
where
    T: GetEntry<T, D>,
{
    unsafe {
        let mut tmp: *mut T = (*head).rbh_root;

        while !tmp.is_null() {
            tmp = match T::cmp(&*elm, &*tmp) {
                Ordering::Less => rb_left(tmp),
                Ordering::Greater => rb_right(tmp),
                Ordering::Equal => return tmp,
            };
        }
    }

    null_mut()
}

pub unsafe fn rb_min<T, D>(head: *mut rb_head<T>) -> *mut T
where
    T: GetEntry<T, D>,
{
    unsafe { rb_minmax(head, -1) }
}

pub unsafe fn rb_max<T, D>(head: *mut rb_head<T>) -> *mut T
where
    T: GetEntry<T, D>,
{
    unsafe { rb_minmax(head, 1) }
}

pub unsafe fn rb_foreach_const<T, D>(head: *const rb_head<T>) -> ConstRbForwardIterator<T, D>
where
    T: GetEntry<T, D>,
{
    ConstRbForwardIterator {
        // TODO being a bit lazy reusing NonNull
        curr: NonNull::new(unsafe { rb_minmax_const(head, -1).cast_mut() }),
        _phantom: std::marker::PhantomData,
    }
}
pub struct ConstRbForwardIterator<T, D> {
    curr: Option<NonNull<T>>,
    _phantom: std::marker::PhantomData<D>,
}

impl<T, D> Iterator for ConstRbForwardIterator<T, D>
where
    T: GetEntry<T, D>,
{
    type Item = NonNull<T>;
    fn next(&mut self) -> Option<Self::Item> {
        let curr = self.curr?.as_ptr();
        std::mem::replace(&mut self.curr, NonNull::new(unsafe { rb_next(curr) }))
    }
}

pub unsafe fn rb_foreach<T, D>(head: *mut rb_head<T>) -> RbForwardIterator<T, D>
where
    T: GetEntry<T, D>,
{
    RbForwardIterator {
        curr: NonNull::new(unsafe { rb_min(head) }),
        _phantom: std::marker::PhantomData,
    }
}
pub struct RbForwardIterator<T, D> {
    curr: Option<NonNull<T>>,
    _phantom: std::marker::PhantomData<D>,
}

impl<T, D> Iterator for RbForwardIterator<T, D>
where
    T: GetEntry<T, D>,
{
    type Item = NonNull<T>;
    fn next(&mut self) -> Option<Self::Item> {
        let curr = self.curr?.as_ptr();
        std::mem::replace(&mut self.curr, NonNull::new(unsafe { rb_next(curr) }))
    }
}

pub unsafe fn rb_foreach_reverse<T, D>(head: *mut rb_head<T>) -> RbReverseIterator<T, D>
where
    T: GetEntry<T, D>,
{
    RbReverseIterator {
        curr: NonNull::new(unsafe { rb_max(head) }),
        _phantom: std::marker::PhantomData,
    }
}

pub struct RbReverseIterator<T, D> {
    curr: Option<NonNull<T>>,
    _phantom: std::marker::PhantomData<D>,
}

impl<T, D> Iterator for RbReverseIterator<T, D>
where
    T: GetEntry<T, D>,
{
    type Item = NonNull<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let curr = self.curr?.as_ptr();
        std::mem::replace(&mut self.curr, NonNull::new(unsafe { rb_prev(curr) }))
    }
}

pub unsafe fn rb_next<T, D>(mut elm: *mut T) -> *mut T
where
    T: GetEntry<T, D>,
{
    unsafe {
        if !rb_right(elm).is_null() {
            elm = rb_right(elm);
            while !rb_left(elm).is_null() {
                elm = rb_left(elm);
            }
        } else if !rb_parent(elm).is_null() && is_left_sibling(elm) {
            elm = rb_parent(elm);
        } else {
            while !rb_parent(elm).is_null() && is_right_sibling(elm) {
                elm = rb_parent(elm);
            }
            elm = rb_parent(elm);
        }

        elm
    }
}

pub unsafe fn rb_prev<T, D>(mut elm: *mut T) -> *mut T
where
    T: GetEntry<T, D>,
{
    unsafe {
        if !rb_left(elm).is_null() {
            elm = rb_left(elm);
            while !rb_right(elm).is_null() {
                elm = rb_right(elm);
            }
        } else if !rb_parent(elm).is_null() && is_right_sibling(elm) {
            elm = rb_parent(elm);
        } else {
            while !rb_parent(elm).is_null() && is_left_sibling(elm) {
                elm = rb_parent(elm);
            }
            elm = rb_parent(elm);
        }

        elm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal RB-tree node keyed by an i32, used to exercise the ported
    // insert/remove rebalancing in isolation (no server globals needed).
    struct Node {
        key: i32,
        entry: rb_entry<Node>,
    }

    impl GetEntry<Node, ()> for Node {
        unsafe fn entry_mut(this: *mut Self) -> *mut rb_entry<Node> {
            unsafe { &raw mut (*this).entry }
        }
        unsafe fn entry_const(this: *const Self) -> *const rb_entry<Node> {
            unsafe { &raw const (*this).entry }
        }
        fn cmp(a: &Self, b: &Self) -> Ordering {
            a.key.cmp(&b.key)
        }
    }

    unsafe fn make(key: i32) -> *mut Node {
        Box::into_raw(Box::new(Node {
            key,
            entry: rb_entry::default(),
        }))
    }

    unsafe fn find(head: *mut rb_head<Node>, key: i32) -> *mut Node {
        unsafe { rb_find_by(head, |n: &Node| key.cmp(&n.key)) }
    }

    // Recursively assert the red-black invariants and return this subtree's
    // black-height. A null child is a black leaf (height 1). This is exactly
    // what silently broke before the rb_remove_color rotation-pivot fix: the
    // tree stayed "usable" for some shapes but corrupted parent/child links for
    // others, eventually dereferencing a bogus node.
    unsafe fn black_height(n: *mut Node) -> i32 {
        unsafe {
            if n.is_null() {
                return 1;
            }
            let l = rb_left(n);
            let r = rb_right(n);
            if rb_color(n) == rb_color::RB_RED {
                assert!(l.is_null() || rb_color(l) == rb_color::RB_BLACK, "red-red left");
                assert!(r.is_null() || rb_color(r) == rb_color::RB_BLACK, "red-red right");
            }
            if !l.is_null() {
                assert!((*l).key < (*n).key, "BST order (left)");
                assert!(rb_parent(l) == n, "left child parent link");
            }
            if !r.is_null() {
                assert!((*r).key > (*n).key, "BST order (right)");
                assert!(rb_parent(r) == n, "right child parent link");
            }
            let bl = black_height(l);
            let br = black_height(r);
            assert_eq!(bl, br, "black-height mismatch at key {}", (*n).key);
            bl + i32::from(rb_color(n) == rb_color::RB_BLACK)
        }
    }

    unsafe fn check(head: *mut rb_head<Node>) {
        unsafe {
            let root = (*head).rbh_root;
            if !root.is_null() {
                assert!(rb_color(root) == rb_color::RB_BLACK, "root must be black");
                assert!(rb_parent(root).is_null(), "root must have no parent");
                black_height(root);
            }
        }
    }

    unsafe fn free_all(head: *mut rb_head<Node>) {
        unsafe {
            let mut n = rb_min(head);
            while !n.is_null() {
                let next = rb_next(n);
                rb_remove(head, n);
                drop(Box::from_raw(n));
                n = next;
            }
        }
    }

    // Reproduces the exact crash: rebinding a key that already exists removes the
    // old node first, and for keys like tmux's `h`/`l` that delete hit the
    // right-hand branch of rb_remove_color, which rotated around the wrong node.
    // Insert h and l, then delete-and-reinsert l (what `bind-key l ...` does).
    #[test]
    fn remove_then_insert_hl_keeps_tree_valid() {
        unsafe {
            let mut head: rb_head<Node> = rb_initializer();
            for k in [b'h' as i32, b'l' as i32] {
                assert!(rb_insert(&raw mut head, make(k)).is_null());
                check(&raw mut head);
            }
            // Emulate rebinding `l`: remove the existing node, insert a fresh one.
            let old = find(&raw mut head, b'l' as i32);
            assert!(!old.is_null());
            rb_remove(&raw mut head, old);
            drop(Box::from_raw(old));
            check(&raw mut head);
            assert!(rb_insert(&raw mut head, make(b'l' as i32)).is_null());
            check(&raw mut head);
            free_all(&raw mut head);
        }
    }

    // Stress the delete rebalancing across many shapes. A deterministic LCG
    // shuffles insert/delete orders so both the left- and right-hand rotation
    // branches of rb_remove_color get exercised. Pre-fix, this panics (invariant
    // break) or segfaults; post-fix it stays a valid red-black tree throughout.
    #[test]
    fn insert_delete_stress_preserves_invariants() {
        unsafe {
            for n in [1usize, 2, 3, 7, 16, 63, 128, 257] {
                let mut head: rb_head<Node> = rb_initializer();
                // Insert 0..n in a pseudo-shuffled order.
                let mut seed: u64 = 0x9E37_79B9_7F4A_7C15 ^ (n as u64);
                let mut order: Vec<i32> = (0..n as i32).collect();
                for i in (1..order.len()).rev() {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    let j = (seed >> 33) as usize % (i + 1);
                    order.swap(i, j);
                }
                for &k in &order {
                    assert!(rb_insert(&raw mut head, make(k)).is_null());
                    check(&raw mut head);
                }
                // Delete in a different pseudo-shuffled order.
                let mut del: Vec<i32> = (0..n as i32).collect();
                for i in (1..del.len()).rev() {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    let j = (seed >> 33) as usize % (i + 1);
                    del.swap(i, j);
                }
                for &k in &del {
                    let node = find(&raw mut head, k);
                    assert!(!node.is_null(), "key {k} should be present");
                    rb_remove(&raw mut head, node);
                    drop(Box::from_raw(node));
                    check(&raw mut head);
                }
                assert!(head.rbh_root.is_null(), "tree should be empty");
            }
        }
    }
}
