use core::ptr::null_mut;
use std::ptr::NonNull;

pub trait ListEntry<T, Discriminant = ()> {
    unsafe fn field(this: *mut Self) -> *mut list_entry<T>;
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct list_head<T> {
    pub lh_first: *mut T,
}
pub const fn list_head_initializer<T>() -> list_head<T> {
    list_head {
        lh_first: null_mut(),
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct list_entry<T> {
    pub le_next: *mut T,
    pub le_prev: *mut *mut T,
}

impl<T> Default for list_entry<T> {
    fn default() -> Self {
        Self { le_next: Default::default(), le_prev: Default::default() }
    }
}

pub unsafe fn list_first<T>(head: *mut list_head<T>) -> *mut T {
    unsafe { (*head).lh_first }
}


pub unsafe fn list_next<T, Discriminant>(elm: *mut T) -> *mut T
where
    T: ListEntry<T, Discriminant>,
{
    unsafe { (*ListEntry::field(elm)).le_next }
}

pub unsafe fn list_foreach<T, D>(head: *mut list_head<T>) -> ListIterator<T, D>
where
    T: ListEntry<T, D>,
{
    ListIterator {
        curr: unsafe { NonNull::new(list_first(head)) },
        _phantom: std::marker::PhantomData,
    }
}

// this implementation can be used in place of safe and non-safe
pub struct ListIterator<T, D> {
    curr: Option<NonNull<T>>,
    _phantom: std::marker::PhantomData<D>,
}
impl<T, D> Iterator for ListIterator<T, D>
where
    T: ListEntry<T, D>,
{
    type Item = NonNull<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let curr = self.curr?.as_ptr();
        std::mem::replace(&mut self.curr, NonNull::new(unsafe { list_next(curr) }))
    }
}

pub unsafe fn list_insert_head<T, D>(head: *mut list_head<T>, elm: *mut T)
where
    T: ListEntry<T, D>,
{
    unsafe {
        (*ListEntry::field(elm)).le_next = (*head).lh_first;
        if !(*ListEntry::field(elm)).le_next.is_null() {
            (*ListEntry::field((*head).lh_first)).le_prev =
                &raw mut (*ListEntry::field(elm)).le_next;
        }
        (*head).lh_first = elm;
        (*ListEntry::field(elm)).le_prev = &raw mut (*head).lh_first;
    }
}

pub unsafe fn list_remove<T, D>(elm: *mut T)
where
    T: ListEntry<T, D>,
{
    unsafe {
        if !(*ListEntry::field(elm)).le_next.is_null() {
            (*ListEntry::field((*ListEntry::field(elm)).le_next)).le_prev =
                (*ListEntry::field(elm)).le_prev;
        }
        *(*ListEntry::field(elm)).le_prev = (*ListEntry::field(elm)).le_next;
    }
}

// tailq

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct tailq_head<T> {
    pub tqh_first: *mut T,
    pub tqh_last: *mut *mut T,
}

macro_rules! TAILQ_HEAD_INITIALIZER {
    ($ident:ident) => {
        $crate::compat::queue::tailq_head {
            tqh_first: null_mut(),
            tqh_last: unsafe { &raw mut $ident.tqh_first },
        }
    };
}
pub(crate) use TAILQ_HEAD_INITIALIZER;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct tailq_entry<T> {
    pub tqe_next: *mut T,
    pub tqe_prev: *mut *mut T,
}

impl<T> Default for tailq_entry<T> {
    fn default() -> Self {
        Self {
            tqe_next: null_mut(),
            tqe_prev: null_mut(),
        }
    }
}

impl<T> std::fmt::Debug for tailq_entry<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("tailq_entry")
            .field("tqe_next", &self.tqe_next)
            .field("tqe_prev", &self.tqe_prev)
            .finish()
    }
}

pub trait Entry<T, Discriminant = ()> {
    unsafe fn entry(this: *mut Self) -> *mut tailq_entry<T>;
}

pub unsafe fn tailq_init<T>(head: *mut tailq_head<T>) {
    unsafe {
        (*head).tqh_first = core::ptr::null_mut();
        (*head).tqh_last = &raw mut (*head).tqh_first;
    }
}

pub fn tailq_init_<T>(head: &mut tailq_head<T>) {
    head.tqh_first = core::ptr::null_mut();
    head.tqh_last = &raw mut head.tqh_first;
}

pub unsafe fn tailq_first<T>(head: *mut tailq_head<T>) -> *mut T {
    unsafe { (*head).tqh_first }
}

pub unsafe fn tailq_next<T, Q, D>(elm: *mut T) -> *mut Q
where
    T: Entry<Q, D>,
{
    unsafe { (*Entry::entry(elm)).tqe_next }
}

pub unsafe fn tailq_last<T>(head: *mut tailq_head<T>) -> *mut T {
    unsafe { *(*(*head).tqh_last.cast::<tailq_head<T>>()).tqh_last }
}

pub unsafe fn tailq_prev<T, Q, D>(elm: *mut T) -> *mut Q
where
    T: Entry<Q, D>,
{
    unsafe {
        let head: *mut tailq_head<Q> = (*Entry::entry(elm)).tqe_prev.cast();
        *(*head).tqh_last
    }
}

pub unsafe fn tailq_empty<T>(head: *const tailq_head<T>) -> bool {
    unsafe { (*head).tqh_first.is_null() }
}

pub unsafe fn tailq_insert_head<T, D>(head: *mut tailq_head<T>, elm: *mut T)
where
    T: Entry<T, D>,
{
    unsafe {
        (*T::entry(elm)).tqe_next = (*head).tqh_first;

        if !(*T::entry(elm)).tqe_next.is_null() {
            (*T::entry((*head).tqh_first)).tqe_prev = &raw mut (*T::entry(elm)).tqe_next;
        } else {
            (*head).tqh_last = &raw mut (*T::entry(elm)).tqe_next;
        }

        (*head).tqh_first = elm;
        (*T::entry(elm)).tqe_prev = &raw mut (*head).tqh_first;
    }
}

pub unsafe fn tailq_insert_tail<T, D>(head: *mut tailq_head<T>, elm: *mut T)
where
    T: Entry<T, D>,
{
    unsafe {
        (*Entry::<_, D>::entry(elm)).tqe_next = null_mut();
        (*Entry::<_, D>::entry(elm)).tqe_prev = (*head).tqh_last;
        *(*head).tqh_last = elm;
        (*head).tqh_last = &raw mut (*Entry::<_, D>::entry(elm)).tqe_next;
    }
}

pub unsafe fn tailq_insert_after<T, D>(head: *mut tailq_head<T>, listelm: *mut T, elm: *mut T)
where
    T: Entry<T, D>,
{
    unsafe {
        (*T::entry(elm)).tqe_next = (*T::entry(listelm)).tqe_next;

        if !(*T::entry(elm)).tqe_next.is_null() {
            (*T::entry((*T::entry(elm)).tqe_next)).tqe_prev = &raw mut (*T::entry(elm)).tqe_next;
        } else {
            (*head).tqh_last = &raw mut (*T::entry(elm)).tqe_next;
        }

        (*T::entry(listelm)).tqe_next = elm;
        (*T::entry(elm)).tqe_prev = &raw mut (*T::entry(listelm)).tqe_next;
    }
}

pub unsafe fn tailq_insert_before<T, D>(listelm: *mut T, elm: *mut T)
where
    T: Entry<T, D>,
{
    unsafe {
        (*T::entry(elm)).tqe_prev = (*T::entry(listelm)).tqe_prev;
        (*T::entry(elm)).tqe_next = listelm;
        *(*T::entry(listelm)).tqe_prev = elm;
        (*T::entry(listelm)).tqe_prev = &raw mut (*T::entry(elm)).tqe_next;
    }
}

pub unsafe fn tailq_remove<T, D>(head: *mut tailq_head<T>, elm: *mut T)
where
    T: Entry<T, D>,
{
    unsafe {
        if !(*Entry::<_, D>::entry(elm)).tqe_next.is_null() {
            (*Entry::<_, D>::entry((*Entry::<_, D>::entry(elm)).tqe_next)).tqe_prev =
                (*Entry::<_, D>::entry(elm)).tqe_prev;
        } else {
            (*head).tqh_last = (*Entry::<_, D>::entry(elm)).tqe_prev;
        }
        *(*Entry::<_, D>::entry(elm)).tqe_prev = (*Entry::<_, D>::entry(elm)).tqe_next;
    }
}

pub unsafe fn tailq_replace<T, D>(head: *mut tailq_head<T>, elm: *mut T, elm2: *mut T)
where
    T: Entry<T, D>,
{
    unsafe {
        (*Entry::<_, D>::entry(elm2)).tqe_next = (*Entry::<_, D>::entry(elm)).tqe_next;
        if !(*Entry::<_, D>::entry(elm2)).tqe_next.is_null() {
            (*Entry::<_, D>::entry((*Entry::<_, D>::entry(elm2)).tqe_next)).tqe_prev =
                &raw mut (*Entry::<_, D>::entry(elm2)).tqe_next;
        } else {
            (*head).tqh_last = &raw mut (*Entry::<_, D>::entry(elm2)).tqe_next;
        }
        (*Entry::<_, D>::entry(elm2)).tqe_prev = (*Entry::<_, D>::entry(elm)).tqe_prev;
        *(*Entry::<_, D>::entry(elm2)).tqe_prev = elm2;
    }
}

pub unsafe fn tailq_foreach_const<T, D>(
    head: *const tailq_head<T>,
) -> ConstTailqForwardIterator<T, D>
where
    T: Entry<T, D>,
{
    unsafe {
        ConstTailqForwardIterator {
            curr: NonNull::new((*head).tqh_first),
            _phantom: std::marker::PhantomData,
        }
    }
}
// this implementation can be used in place of safe and non-safe
pub struct ConstTailqForwardIterator<T, D> {
    curr: Option<NonNull<T>>,
    _phantom: std::marker::PhantomData<D>,
}
impl<T, D> Iterator for ConstTailqForwardIterator<T, D>
where
    T: Entry<T, D>,
{
    type Item = NonNull<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let curr = self.curr?.as_ptr();
        std::mem::replace(&mut self.curr, NonNull::new(unsafe { tailq_next(curr) }))
    }
}

pub unsafe fn tailq_foreach<T, D>(head: *mut tailq_head<T>) -> TailqForwardIterator<T, D>
where
    T: Entry<T, D>,
{
    unsafe {
        TailqForwardIterator {
            curr: NonNull::new(tailq_first(head)),
            _phantom: std::marker::PhantomData,
        }
    }
}

// this implementation can be used in place of safe and non-safe
pub struct TailqForwardIterator<T, D> {
    curr: Option<NonNull<T>>,
    _phantom: std::marker::PhantomData<D>,
}
impl<T, D> Iterator for TailqForwardIterator<T, D>
where
    T: Entry<T, D>,
{
    type Item = NonNull<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let curr = self.curr?.as_ptr();
        std::mem::replace(&mut self.curr, NonNull::new(unsafe { tailq_next(curr) }))
    }
}

pub unsafe fn tailq_foreach_reverse<T, D>(head: *mut tailq_head<T>) -> TailqReverseIterator<T, D>
where
    T: Entry<T, D>,
{
    unsafe {
        TailqReverseIterator {
            curr: NonNull::new(tailq_last(head)),
            _phantom: std::marker::PhantomData,
        }
    }
}

// this implementation can be used in place of safe and non-safe
pub struct TailqReverseIterator<T, D> {
    curr: Option<NonNull<T>>,
    _phantom: std::marker::PhantomData<D>,
}
impl<T, D> Iterator for TailqReverseIterator<T, D>
where
    T: Entry<T, D>,
{
    type Item = NonNull<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let curr = self.curr?.as_ptr();
        std::mem::replace(&mut self.curr, NonNull::new(unsafe { tailq_prev(curr) }))
    }
}

#[inline]
pub unsafe fn tailq_concat<T, D>(head1: *mut tailq_head<T>, head2: *mut tailq_head<T>)
where
    T: Entry<T, D>,
{
    unsafe {
        if !tailq_empty::<T>(head2) {
            *(*head1).tqh_last = (*head2).tqh_first;
            (*Entry::entry((*head2).tqh_first)).tqe_prev = (*head1).tqh_last;
            (*head1).tqh_last = (*head2).tqh_last;
            tailq_init(head2);
        }
    }
}

macro_rules! impl_tailq_entry {
    ($struct_name:ident, $attribute_field_name:ident, $attribute_field_ty:ty) => {
        impl $crate::compat::queue::Entry<$struct_name> for $struct_name {
            unsafe fn entry(this: *mut Self) -> *mut $attribute_field_ty {
                unsafe { &raw mut (*this).$attribute_field_name }
            }
        }
    };
}
pub(crate) use impl_tailq_entry;

#[cfg(test)]
mod tests {
    use super::*;

    // ---- TAILQ test fixtures --------------------------------------------
    //
    // A minimal doubly-linked tail-queue node keyed by an i32. This mirrors the
    // C `TAILQ_ENTRY(type)` pattern from vendor/tmux/compat/queue.h (lines
    // 416-420) where each node embeds a `{ tqe_next; tqe_prev; }`. We drive the
    // ported TAILQ_* ops in isolation with no dependency on server globals.
    #[repr(C)]
    struct TNode {
        key: i32,
        entry: tailq_entry<TNode>,
    }

    impl Entry<TNode> for TNode {
        unsafe fn entry(this: *mut Self) -> *mut tailq_entry<TNode> {
            unsafe { &raw mut (*this).entry }
        }
    }

    unsafe fn tmake(key: i32) -> *mut TNode {
        Box::into_raw(Box::new(TNode {
            key,
            entry: tailq_entry::default(),
        }))
    }

    // Collect the keys by walking TAILQ_FIRST / TAILQ_NEXT (queue.h 425, 427).
    unsafe fn tkeys(head: *mut tailq_head<TNode>) -> Vec<i32> {
        unsafe {
            let mut out = Vec::new();
            let mut n = tailq_first(head);
            while !n.is_null() {
                out.push((*n).key);
                n = tailq_next::<_, TNode, ()>(n);
            }
            out
        }
    }

    // Collect keys walking the forward iterator (TAILQ_FOREACH, queue.h 436).
    unsafe fn tkeys_foreach(head: *mut tailq_head<TNode>) -> Vec<i32> {
        unsafe { tailq_foreach::<TNode, ()>(head).map(|p| (*p.as_ptr()).key).collect() }
    }

    // Collect keys walking backwards (TAILQ_FOREACH_REVERSE, queue.h 448).
    unsafe fn tkeys_reverse(head: *mut tailq_head<TNode>) -> Vec<i32> {
        unsafe {
            tailq_foreach_reverse::<TNode, ()>(head)
                .map(|p| (*p.as_ptr()).key)
                .collect()
        }
    }

    unsafe fn tfree_all(head: *mut tailq_head<TNode>) {
        unsafe {
            let mut n = tailq_first(head);
            while !n.is_null() {
                let next = tailq_next::<_, TNode, ()>(n);
                drop(Box::from_raw(n));
                n = next;
            }
            tailq_init(head);
        }
    }

    // TAILQ_INIT (queue.h 462) leaves an empty queue: first is NULL and last
    // points back at first. TAILQ_EMPTY (queue.h 433) must be true.
    #[test]
    fn tailq_init_makes_empty() {
        unsafe {
            let mut head: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut head);
            assert!(tailq_empty(&raw const head));
            assert!(head.tqh_first.is_null());
            assert_eq!(head.tqh_last, &raw mut head.tqh_first);
            assert!(tailq_first(&raw mut head).is_null());
            // last on empty is NULL, so both iterators yield nothing.
            assert!(tailq_last(&raw mut head).is_null());
            assert!(tkeys_foreach(&raw mut head).is_empty());
            assert!(tkeys_reverse(&raw mut head).is_empty());
        }
    }

    // tailq_init_ is the &mut-taking twin of tailq_init; same postconditions.
    #[test]
    fn tailq_init_ref_makes_empty() {
        let mut head: tailq_head<TNode> = tailq_head {
            tqh_first: null_mut(),
            tqh_last: null_mut(),
        };
        tailq_init_(&mut head);
        assert!(head.tqh_first.is_null());
        assert_eq!(head.tqh_last, &raw mut head.tqh_first);
    }

    // TAILQ_INSERT_HEAD (queue.h 467) prepends: inserting 1 then 2 then 3 yields
    // front-to-back order 3,2,1. first/last and both traversal directions agree.
    #[test]
    fn tailq_insert_head_prepends() {
        unsafe {
            let mut head: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut head);
            for k in [1, 2, 3] {
                tailq_insert_head(&raw mut head, tmake(k));
            }
            assert!(!tailq_empty(&raw const head));
            assert_eq!(tkeys(&raw mut head), vec![3, 2, 1]);
            assert_eq!(tkeys_foreach(&raw mut head), vec![3, 2, 1]);
            assert_eq!(tkeys_reverse(&raw mut head), vec![1, 2, 3]);
            assert_eq!((*tailq_first(&raw mut head)).key, 3);
            assert_eq!((*tailq_last(&raw mut head)).key, 1);
            tfree_all(&raw mut head);
        }
    }

    // TAILQ_INSERT_TAIL (queue.h 477) appends: 1,2,3 stays 1,2,3. Exercises the
    // tqh_last bookkeeping that keeps the reverse walk consistent.
    #[test]
    fn tailq_insert_tail_appends() {
        unsafe {
            let mut head: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut head);
            for k in [1, 2, 3] {
                tailq_insert_tail(&raw mut head, tmake(k));
            }
            assert_eq!(tkeys(&raw mut head), vec![1, 2, 3]);
            assert_eq!(tkeys_foreach(&raw mut head), vec![1, 2, 3]);
            assert_eq!(tkeys_reverse(&raw mut head), vec![3, 2, 1]);
            assert_eq!((*tailq_first(&raw mut head)).key, 1);
            assert_eq!((*tailq_last(&raw mut head)).key, 3);
            tfree_all(&raw mut head);
        }
    }

    // TAILQ_INSERT_AFTER (queue.h 484): insert after a middle element and after
    // the current tail. The after-tail case must update tqh_last so the reverse
    // walk still starts at the new tail.
    #[test]
    fn tailq_insert_after_middle_and_tail() {
        unsafe {
            let mut head: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut head);
            let a = tmake(1);
            let c = tmake(3);
            tailq_insert_tail(&raw mut head, a);
            tailq_insert_tail(&raw mut head, c);
            // after 'a' (middle insert): 1,2,3
            tailq_insert_after(&raw mut head, a, tmake(2));
            assert_eq!(tkeys(&raw mut head), vec![1, 2, 3]);
            // after 'c' (tail insert): 1,2,3,4
            tailq_insert_after(&raw mut head, c, tmake(4));
            assert_eq!(tkeys(&raw mut head), vec![1, 2, 3, 4]);
            assert_eq!((*tailq_last(&raw mut head)).key, 4);
            assert_eq!(tkeys_reverse(&raw mut head), vec![4, 3, 2, 1]);
            tfree_all(&raw mut head);
        }
    }

    // TAILQ_INSERT_BEFORE (queue.h 494): insert before a middle element and
    // before the head. Before-head effectively becomes the new first.
    #[test]
    fn tailq_insert_before_middle_and_head() {
        unsafe {
            let mut head: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut head);
            let a = tmake(2);
            let b = tmake(4);
            tailq_insert_tail(&raw mut head, a);
            tailq_insert_tail(&raw mut head, b);
            // before 'b': 2,3,4
            tailq_insert_before(b, tmake(3));
            assert_eq!(tkeys(&raw mut head), vec![2, 3, 4]);
            // before 'a' (the head): 1,2,3,4
            tailq_insert_before(a, tmake(1));
            assert_eq!(tkeys(&raw mut head), vec![1, 2, 3, 4]);
            assert_eq!((*tailq_first(&raw mut head)).key, 1);
            assert_eq!(tkeys_reverse(&raw mut head), vec![4, 3, 2, 1]);
            tfree_all(&raw mut head);
        }
    }

    // TAILQ_NEXT / TAILQ_PREV (queue.h 427, 431) navigation over a built queue.
    #[test]
    fn tailq_next_prev_navigation() {
        unsafe {
            let mut head: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut head);
            let n1 = tmake(10);
            let n2 = tmake(20);
            let n3 = tmake(30);
            tailq_insert_tail(&raw mut head, n1);
            tailq_insert_tail(&raw mut head, n2);
            tailq_insert_tail(&raw mut head, n3);

            assert_eq!((*tailq_next::<_, TNode, ()>(n1)).key, 20);
            assert_eq!((*tailq_next::<_, TNode, ()>(n2)).key, 30);
            assert!(tailq_next::<_, TNode, ()>(n3).is_null());

            assert_eq!((*tailq_prev::<_, TNode, ()>(n3)).key, 20);
            assert_eq!((*tailq_prev::<_, TNode, ()>(n2)).key, 10);
            tfree_all(&raw mut head);
        }
    }

    // TAILQ_REMOVE (queue.h 501) for the three distinct branches: removing the
    // head, a middle node, and the tail (the else branch that fixes tqh_last).
    #[test]
    fn tailq_remove_head_middle_tail() {
        unsafe {
            let mut head: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut head);
            let n1 = tmake(1);
            let n2 = tmake(2);
            let n3 = tmake(3);
            let n4 = tmake(4);
            for n in [n1, n2, n3, n4] {
                tailq_insert_tail(&raw mut head, n);
            }
            // remove middle
            tailq_remove(&raw mut head, n2);
            drop(Box::from_raw(n2));
            assert_eq!(tkeys(&raw mut head), vec![1, 3, 4]);
            assert_eq!(tkeys_reverse(&raw mut head), vec![4, 3, 1]);
            // remove tail (else branch updates tqh_last)
            tailq_remove(&raw mut head, n4);
            drop(Box::from_raw(n4));
            assert_eq!(tkeys(&raw mut head), vec![1, 3]);
            assert_eq!((*tailq_last(&raw mut head)).key, 3);
            assert_eq!(tkeys_reverse(&raw mut head), vec![3, 1]);
            // remove head
            tailq_remove(&raw mut head, n1);
            drop(Box::from_raw(n1));
            assert_eq!(tkeys(&raw mut head), vec![3]);
            assert_eq!((*tailq_first(&raw mut head)).key, 3);
            // remove last remaining -> empty
            tailq_remove(&raw mut head, n3);
            drop(Box::from_raw(n3));
            assert!(tailq_empty(&raw const head));
            assert_eq!(head.tqh_last, &raw mut head.tqh_first);
        }
    }

    // TAILQ_REPLACE (queue.h 512): swap a node in place, both middle and tail.
    #[test]
    fn tailq_replace_middle_and_tail() {
        unsafe {
            let mut head: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut head);
            let n1 = tmake(1);
            let n2 = tmake(2);
            let n3 = tmake(3);
            for n in [n1, n2, n3] {
                tailq_insert_tail(&raw mut head, n);
            }
            // replace middle n2 with key 20
            let r = tmake(20);
            tailq_replace(&raw mut head, n2, r);
            drop(Box::from_raw(n2));
            assert_eq!(tkeys(&raw mut head), vec![1, 20, 3]);
            assert_eq!(tkeys_reverse(&raw mut head), vec![3, 20, 1]);
            // replace tail n3 with key 30 (else branch updates tqh_last)
            let r2 = tmake(30);
            tailq_replace(&raw mut head, n3, r2);
            drop(Box::from_raw(n3));
            assert_eq!(tkeys(&raw mut head), vec![1, 20, 30]);
            assert_eq!((*tailq_last(&raw mut head)).key, 30);
            assert_eq!(tkeys_reverse(&raw mut head), vec![30, 20, 1]);
            tfree_all(&raw mut head);
        }
    }

    // TAILQ_CONCAT (queue.h 524): splice head2 onto the end of head1 and leave
    // head2 empty. Also verify the no-op path when head2 is empty.
    #[test]
    fn tailq_concat_splices_and_empties() {
        unsafe {
            let mut h1: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            let mut h2: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut h1);
            tailq_init(&raw mut h2);
            for k in [1, 2] {
                tailq_insert_tail(&raw mut h1, tmake(k));
            }
            for k in [3, 4] {
                tailq_insert_tail(&raw mut h2, tmake(k));
            }
            tailq_concat(&raw mut h1, &raw mut h2);
            assert_eq!(tkeys(&raw mut h1), vec![1, 2, 3, 4]);
            assert_eq!(tkeys_reverse(&raw mut h1), vec![4, 3, 2, 1]);
            assert!(tailq_empty(&raw const h2));
            assert_eq!(h2.tqh_last, &raw mut h2.tqh_first);

            // concat with an empty head2 is a no-op.
            let mut h3: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut h3);
            tailq_concat(&raw mut h1, &raw mut h3);
            assert_eq!(tkeys(&raw mut h1), vec![1, 2, 3, 4]);
            tfree_all(&raw mut h1);
        }
    }

    // The const forward iterator (TAILQ_FOREACH over a *const head) must visit
    // the same nodes in the same order as the mutable one.
    #[test]
    fn tailq_foreach_const_matches() {
        unsafe {
            let mut head: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut head);
            for k in [5, 6, 7] {
                tailq_insert_tail(&raw mut head, tmake(k));
            }
            let got: Vec<i32> = tailq_foreach_const::<TNode, ()>(&raw const head)
                .map(|p| (*p.as_ptr()).key)
                .collect();
            assert_eq!(got, vec![5, 6, 7]);
            tfree_all(&raw mut head);
        }
    }

    // ---- LIST test fixtures ---------------------------------------------
    //
    // Doubly-linked LIST node (LIST_ENTRY, queue.h 172-176) with le_next and
    // le_prev. Only head-insert plus arbitrary remove and forward traversal are
    // ported, matching what the crate uses.
    #[repr(C)]
    struct LNode {
        key: i32,
        entry: list_entry<LNode>,
    }

    impl ListEntry<LNode> for LNode {
        unsafe fn field(this: *mut Self) -> *mut list_entry<LNode> {
            unsafe { &raw mut (*this).entry }
        }
    }

    unsafe fn lmake(key: i32) -> *mut LNode {
        Box::into_raw(Box::new(LNode {
            key,
            entry: list_entry::default(),
        }))
    }

    unsafe fn lkeys(head: *mut list_head<LNode>) -> Vec<i32> {
        unsafe {
            let mut out = Vec::new();
            let mut n = list_first(head);
            while !n.is_null() {
                out.push((*n).key);
                n = list_next::<_, ()>(n);
            }
            out
        }
    }

    unsafe fn lfree_all(head: *mut list_head<LNode>) {
        unsafe {
            let mut n = list_first(head);
            while !n.is_null() {
                let next = list_next::<_, ()>(n);
                drop(Box::from_raw(n));
                n = next;
            }
            (*head).lh_first = null_mut();
        }
    }

    // LIST_HEAD_INITIALIZER / LIST_INSERT_HEAD (queue.h 169, 218): the head list
    // starts empty (lh_first NULL) and prepends, so 1,2,3 becomes 3,2,1.
    #[test]
    fn list_insert_head_prepends() {
        unsafe {
            let mut head: list_head<LNode> = list_head_initializer();
            assert!(list_first(&raw mut head).is_null());
            for k in [1, 2, 3] {
                list_insert_head(&raw mut head, lmake(k));
            }
            assert_eq!(lkeys(&raw mut head), vec![3, 2, 1]);
            let got: Vec<i32> = list_foreach::<LNode, ()>(&raw mut head)
                .map(|p| (*p.as_ptr()).key)
                .collect();
            assert_eq!(got, vec![3, 2, 1]);
            assert_eq!((*list_first(&raw mut head)).key, 3);
            lfree_all(&raw mut head);
        }
    }

    // LIST_REMOVE (queue.h 225) for head, middle and tail. le_prev indirection
    // must keep the chain intact after each removal.
    #[test]
    fn list_remove_head_middle_tail() {
        unsafe {
            let mut head: list_head<LNode> = list_head_initializer();
            // insert_head of 1,2,3,4 -> order is 4,3,2,1
            let n1 = lmake(1);
            let n2 = lmake(2);
            let n3 = lmake(3);
            let n4 = lmake(4);
            for n in [n1, n2, n3, n4] {
                list_insert_head(&raw mut head, n);
            }
            assert_eq!(lkeys(&raw mut head), vec![4, 3, 2, 1]);
            // remove middle (n3): 4,2,1
            list_remove::<_, ()>(n3);
            drop(Box::from_raw(n3));
            assert_eq!(lkeys(&raw mut head), vec![4, 2, 1]);
            // remove head (n4): 2,1
            list_remove::<_, ()>(n4);
            drop(Box::from_raw(n4));
            assert_eq!(lkeys(&raw mut head), vec![2, 1]);
            assert_eq!((*list_first(&raw mut head)).key, 2);
            // remove tail (n1): 2
            list_remove::<_, ()>(n1);
            drop(Box::from_raw(n1));
            assert_eq!(lkeys(&raw mut head), vec![2]);
            // remove last: empty
            list_remove::<_, ()>(n2);
            drop(Box::from_raw(n2));
            assert!(list_first(&raw mut head).is_null());
        }
    }

    // LIST_NEXT (queue.h 184): forward navigation, next of the tail is NULL.
    #[test]
    fn list_next_navigation() {
        unsafe {
            let mut head: list_head<LNode> = list_head_initializer();
            let a = lmake(1);
            let b = lmake(2);
            list_insert_head(&raw mut head, b);
            list_insert_head(&raw mut head, a); // order: 1,2
            assert_eq!((*list_next::<_, ()>(a)).key, 2);
            assert!(list_next::<_, ()>(b).is_null());
            lfree_all(&raw mut head);
        }
    }

    // A single-element TAILQ: first == last, TAILQ_NEXT/TAILQ_PREV are NULL
    // (queue.h 425/427/431), and both traversal directions yield one key.
    #[test]
    fn tailq_single_element() {
        unsafe {
            let mut head: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut head);
            let n = tmake(99);
            tailq_insert_tail(&raw mut head, n);
            assert_eq!(tailq_first(&raw mut head), n);
            assert_eq!(tailq_last(&raw mut head), n);
            assert!(tailq_next::<_, TNode, ()>(n).is_null());
            assert!(tailq_prev::<_, TNode, ()>(n).is_null());
            assert_eq!(tkeys_foreach(&raw mut head), vec![99]);
            assert_eq!(tkeys_reverse(&raw mut head), vec![99]);
            tfree_all(&raw mut head);
        }
    }

    // Removing the head node repeatedly keeps forward and reverse traversals
    // consistent at every step and finally restores the empty invariant
    // (tqh_last points back at tqh_first, queue.h 462/501).
    #[test]
    fn tailq_remove_head_until_empty() {
        unsafe {
            let mut head: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut head);
            let nodes: Vec<*mut TNode> = (0..5).map(|k| tmake(k)).collect();
            for &n in &nodes {
                tailq_insert_tail(&raw mut head, n);
            }
            for expected_first in 0..5 {
                assert_eq!((*tailq_first(&raw mut head)).key, expected_first);
                let fwd = tkeys_foreach(&raw mut head);
                let mut rev = tkeys_reverse(&raw mut head);
                rev.reverse();
                assert_eq!(fwd, rev, "forward and reverse disagree");
                let h = tailq_first(&raw mut head);
                tailq_remove(&raw mut head, h);
                drop(Box::from_raw(h));
            }
            assert!(tailq_empty(&raw const head));
            assert_eq!(head.tqh_last, &raw mut head.tqh_first);
        }
    }

    // TAILQ_REPLACE at the head (queue.h 512): the replacement becomes first
    // and the reverse walk still terminates correctly.
    #[test]
    fn tailq_replace_head() {
        unsafe {
            let mut head: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut head);
            let n1 = tmake(1);
            let n2 = tmake(2);
            tailq_insert_tail(&raw mut head, n1);
            tailq_insert_tail(&raw mut head, n2);
            let r = tmake(10);
            tailq_replace(&raw mut head, n1, r);
            drop(Box::from_raw(n1));
            assert_eq!((*tailq_first(&raw mut head)).key, 10);
            assert_eq!(tkeys(&raw mut head), vec![10, 2]);
            assert_eq!(tkeys_reverse(&raw mut head), vec![2, 10]);
            tfree_all(&raw mut head);
        }
    }

    // TAILQ_PREV of the first element resolves to NULL: its tqe_prev points at
    // the head, whose tqh_last dereferences the first node's (NULL) tqe_next.
    #[test]
    fn tailq_prev_of_first_is_null() {
        unsafe {
            let mut head: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut head);
            let a = tmake(1);
            let b = tmake(2);
            tailq_insert_tail(&raw mut head, a);
            tailq_insert_tail(&raw mut head, b);
            assert!(tailq_prev::<_, TNode, ()>(a).is_null());
            assert_eq!((*tailq_prev::<_, TNode, ()>(b)).key, 1);
            tfree_all(&raw mut head);
        }
    }

    // Bulk TAILQ_INSERT_TAIL of 100 nodes: forward keys are 0..100, reverse is
    // the mirror, and first/last are the endpoints. Threads tqh_last over many
    // ops (queue.h 477).
    #[test]
    fn tailq_insert_tail_bulk() {
        unsafe {
            let mut head: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut head);
            for k in 0..100 {
                tailq_insert_tail(&raw mut head, tmake(k));
            }
            let fwd: Vec<i32> = (0..100).collect();
            let rev: Vec<i32> = (0..100).rev().collect();
            assert_eq!(tkeys(&raw mut head), fwd);
            assert_eq!(tkeys_foreach(&raw mut head), fwd);
            assert_eq!(tkeys_reverse(&raw mut head), rev);
            assert_eq!((*tailq_first(&raw mut head)).key, 0);
            assert_eq!((*tailq_last(&raw mut head)).key, 99);
            tfree_all(&raw mut head);
        }
    }

    // TAILQ_CONCAT onto an empty head1 (queue.h 524) adopts head2's list intact
    // and leaves head2 empty.
    #[test]
    fn tailq_concat_into_empty() {
        unsafe {
            let mut h1: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            let mut h2: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut h1);
            tailq_init(&raw mut h2);
            for k in [7, 8, 9] {
                tailq_insert_tail(&raw mut h2, tmake(k));
            }
            tailq_concat(&raw mut h1, &raw mut h2);
            assert_eq!(tkeys(&raw mut h1), vec![7, 8, 9]);
            assert_eq!(tkeys_reverse(&raw mut h1), vec![9, 8, 7]);
            assert_eq!((*tailq_last(&raw mut h1)).key, 9);
            assert!(tailq_empty(&raw const h2));
            assert_eq!(h2.tqh_last, &raw mut h2.tqh_first);
            tfree_all(&raw mut h1);
        }
    }

    // Repeated TAILQ_INSERT_BEFORE at a fixed anchor (queue.h 494) builds an
    // ascending run in front of the anchor.
    #[test]
    fn tailq_insert_before_builds_ascending() {
        unsafe {
            let mut head: tailq_head<TNode> = tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            };
            tailq_init(&raw mut head);
            let anchor = tmake(100);
            tailq_insert_tail(&raw mut head, anchor);
            for k in [1, 2, 3, 4] {
                tailq_insert_before(anchor, tmake(k));
            }
            assert_eq!(tkeys(&raw mut head), vec![1, 2, 3, 4, 100]);
            assert_eq!(tkeys_reverse(&raw mut head), vec![100, 4, 3, 2, 1]);
            assert_eq!((*tailq_first(&raw mut head)).key, 1);
            tfree_all(&raw mut head);
        }
    }

    // A single-element LIST: LIST_NEXT is NULL (queue.h 184) and the forward
    // walk yields exactly one key.
    #[test]
    fn list_single_element() {
        unsafe {
            let mut head: list_head<LNode> = list_head_initializer();
            let n = lmake(5);
            list_insert_head(&raw mut head, n);
            assert_eq!(list_first(&raw mut head), n);
            assert!(list_next::<_, ()>(n).is_null());
            assert_eq!(lkeys(&raw mut head), vec![5]);
            lfree_all(&raw mut head);
        }
    }

    // An empty LIST: lh_first is NULL and both the raw walk and the iterator
    // yield nothing.
    #[test]
    fn list_empty() {
        unsafe {
            let mut head: list_head<LNode> = list_head_initializer();
            assert!(list_first(&raw mut head).is_null());
            assert!(lkeys(&raw mut head).is_empty());
            let got: Vec<i32> = list_foreach::<LNode, ()>(&raw mut head)
                .map(|p| (*p.as_ptr()).key)
                .collect();
            assert!(got.is_empty());
        }
    }

    // Bulk LIST_INSERT_HEAD of 50 then LIST_REMOVE of every even key: the
    // le_prev back-links must keep the surviving odd chain intact (queue.h
    // 218/225).
    #[test]
    fn list_bulk_insert_and_sparse_remove() {
        unsafe {
            let mut head: list_head<LNode> = list_head_initializer();
            let nodes: Vec<*mut LNode> = (0..50).map(|k| lmake(k)).collect();
            for &n in &nodes {
                list_insert_head(&raw mut head, n);
            }
            // insert_head reverses: keys are 49,48,...,0.
            let all: Vec<i32> = (0..50).rev().collect();
            assert_eq!(lkeys(&raw mut head), all);
            // Remove every even key. nodes[i].key == i, so index parity selects
            // the node without dereferencing (freed nodes must not be read).
            for (i, &n) in nodes.iter().enumerate() {
                if i % 2 == 0 {
                    list_remove::<_, ()>(n);
                    drop(Box::from_raw(n));
                }
            }
            let odds: Vec<i32> = (0..50).rev().filter(|k| k % 2 != 0).collect();
            assert_eq!(lkeys(&raw mut head), odds);
            // Free the survivors (the odd-indexed nodes).
            for (i, &n) in nodes.iter().enumerate() {
                if i % 2 != 0 {
                    list_remove::<_, ()>(n);
                    drop(Box::from_raw(n));
                }
            }
            assert!(list_first(&raw mut head).is_null());
        }
    }

    // Removing the head element updates lh_first through le_prev (queue.h 225),
    // then removing the new head empties the list.
    #[test]
    fn list_remove_head_updates_first() {
        unsafe {
            let mut head: list_head<LNode> = list_head_initializer();
            let a = lmake(1);
            let b = lmake(2);
            list_insert_head(&raw mut head, a);
            list_insert_head(&raw mut head, b); // order: 2,1
            assert_eq!((*list_first(&raw mut head)).key, 2);
            list_remove::<_, ()>(b);
            drop(Box::from_raw(b));
            assert_eq!((*list_first(&raw mut head)).key, 1);
            list_remove::<_, ()>(a);
            drop(Box::from_raw(a));
            assert!(list_first(&raw mut head).is_null());
        }
    }
}
