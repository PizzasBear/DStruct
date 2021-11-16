#![allow(dead_code)]

// TODO: Optimize the number of moves (and copies potentialy?) of StackVec based structs.

use crate::{OnStackRefMutStack, OuterLenStackVec, RefMutStack, StackVec, StackVecIntoIter};
use std::{
    cmp::Ordering,
    fmt, mem,
    ops::{Deref, DerefMut},
    ptr,
};

const B: usize = 6;

const MIN_NUM_ELEMENTS: usize = B - 1;
const MAX_NUM_ELEMENTS: usize = 2 * B - 1;
const MIN_NUM_CHILDREN: usize = B;
const MAX_NUM_CHILDREN: usize = 2 * B;

trait OptionExt {
    fn assert_none(&self);
}

impl<T> OptionExt for Option<T> {
    #[inline]
    fn assert_none(&self) {
        assert!(
            self.is_none(),
            "called `Option::unwrap()` on a `None` value",
        );
    }
}

trait BoundClonedExt {
    type Target: Clone;

    fn cloned(&self) -> std::ops::Bound<Self::Target>;
}

impl<T: Clone> BoundClonedExt for std::ops::Bound<&T> {
    type Target = T;

    fn cloned(&self) -> std::ops::Bound<T> {
        match self {
            Self::Unbounded => std::ops::Bound::Unbounded,
            Self::Included(x) => std::ops::Bound::Included((*x).clone()),
            Self::Excluded(x) => std::ops::Bound::Excluded((*x).clone()),
        }
    }
}

pub trait OrdSize {
    fn size(&self) -> usize;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrdSizeOne;

impl OrdSize for OrdSizeOne {
    #[inline(always)]
    fn size(&self) -> usize {
        1
    }
}

struct NodeElements<K: OrdSize, V> {
    _keys: OuterLenStackVec<K, MAX_NUM_ELEMENTS>,
    _values: OuterLenStackVec<V, MAX_NUM_ELEMENTS>,
    _len: u8,
    size: usize,
    // parent: *mut Node<T>,
}

impl<K: OrdSize, V> NodeElements<K, V> {
    // pub fn new(parent: *mut Node<T>) -> Self {
    //     unsafe { Self::from_raw_parts(OuterLenStackVec::new(), OuterLenStackVec::new(), 0, parent) }
    // }
    pub fn new() -> Self {
        unsafe { Self::from_raw_parts(OuterLenStackVec::new(), OuterLenStackVec::new(), 0, 0) }
    }

    #[inline]
    pub fn keys(&self) -> &[K] {
        unsafe { self._keys.as_slice(self.len()) }
    }

    #[inline]
    pub fn keys_mut(&mut self) -> &mut [K] {
        unsafe { self._keys.as_slice_mut(self.len()) }
    }

    #[inline]
    pub fn values(&self) -> &[V] {
        unsafe { self._values.as_slice(self.len()) }
    }

    #[inline]
    pub fn values_mut(&mut self) -> &mut [V] {
        unsafe { self._values.as_slice_mut(self.len()) }
    }

    /// Get both `keys` and `values` as mutables at the same.
    #[inline]
    pub fn get_all_mut(&mut self) -> (&mut [K], &mut [V]) {
        let len = self.len();
        unsafe { (self._keys.as_slice_mut(len), self._values.as_slice_mut(len)) }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self._len as _
    }

    #[inline(always)]
    pub fn size(&self) -> usize {
        self.size
    }

    #[inline(always)]
    pub unsafe fn set_len(&mut self, len: usize) {
        debug_assert!((0..256).contains(&len));
        self._len = len as _;
    }

    #[inline(always)]
    pub unsafe fn set_size(&mut self, size: usize) {
        self.size = size;
    }

    #[must_use]
    #[inline]
    pub fn push(&mut self, key: K, value: V) -> Option<(K, V)> {
        unsafe {
            self.size += key.size();

            let overflow_key = self._keys.push(&mut self.len(), key);
            let mut len = self.len();
            let overflow_value = self._values.push(&mut len, value);
            self.set_len(len);

            if let Some(overflow_key) = &overflow_value {
                self.size -= overflow_key.size();
            }

            match (overflow_key, overflow_value) {
                (Some(overflow_key), Some(overflow_value)) => Some((overflow_key, overflow_value)),
                (None, None) => None,
                _ => unreachable!(),
            }
        }
    }

    #[must_use]
    pub fn insert(&mut self, idx: usize, key: K, value: V) -> Option<(K, V)> {
        unsafe {
            self.size += key.size();

            let overflow_key = self._keys.insert(&mut self.len(), idx, key);
            let mut len = self.len();
            let overflow_value = self._values.insert(&mut len, idx, value);
            self.set_len(len);

            if let Some(overflow_key) = &overflow_key {
                self.size -= overflow_key.size();
            }

            match (overflow_key, overflow_value) {
                (Some(overflow_key), Some(overflow_value)) => Some((overflow_key, overflow_value)),
                (None, None) => None,
                _ => unreachable!(),
            }
        }
    }

    #[inline]
    pub fn pop(&mut self) -> Option<(K, V)> {
        unsafe {
            let popped_key = self._keys.pop(&mut self.len());
            let mut len = self.len();
            let popped_value = self._values.pop(&mut len);
            self.set_len(len);

            if let Some(popped_key) = &popped_key {
                self.size -= popped_key.size();
            }

            popped_key
        }
    }

    pub fn remove(&mut self, idx: usize) -> (K, V) {
        unsafe {
            let removed_key = self._keys.remove(&mut self.len(), idx);
            let mut len = self.len();
            let removed_value = self._values.remove(&mut len, idx);
            self.set_len(len);

            self.size -= removed_key.size();

            (removed_key, removed_value)
        }
    }

    #[inline]
    fn into_raw_parts(
        self,
    ) -> (
        OuterLenStackVec<K, MAX_NUM_ELEMENTS>,
        OuterLenStackVec<V, MAX_NUM_ELEMENTS>,
        usize,
        usize,
    ) {
        unsafe {
            let mb = mem::ManuallyDrop::new(self);
            (
                ptr::read(&mb._keys),
                ptr::read(&mb._values),
                mb._len as _,
                mb.size,
            )
        }
    }

    #[inline]
    unsafe fn from_raw_parts(
        keys: OuterLenStackVec<K, MAX_NUM_ELEMENTS>,
        values: OuterLenStackVec<V, MAX_NUM_ELEMENTS>,
        len: usize,
        size: usize,
    ) -> Self {
        Self {
            _keys: keys,
            _values: values,
            _len: len as _,
            size,
        }
    }

    // #[inline]
    // pub fn split(&mut self, rightmost_key: K, rightmost_value: V) -> (K, V, Self) {
    //     assert_eq!(self.len(), MAX_NUM_ELEMENTS);
    //     unsafe {
    //         let mut len = self.len();

    //         let mut right_keys = self
    //             ._keys
    //             .split_at(&mut len.clone(), MAX_NUM_ELEMENTS / 2 + 1)
    //             .into_raw_parts()
    //             .0;
    //         let (mut right_values, mut right_len) = self
    //             ._values
    //             .split_at(&mut len, MAX_NUM_ELEMENTS / 2 + 1)
    //             .into_raw_parts();

    //         right_keys
    //             .push(&mut right_len.clone(), rightmost_key)
    //             .assert_none();
    //         right_values
    //             .push(&mut right_len, rightmost_value)
    //             .assert_none();

    //         let sep_k = self._keys.pop(&mut len.clone()).unwrap();
    //         let sep_value = self._values.pop(&mut len).unwrap();

    //         self.set_len(len);

    //         (
    //             sep_k,
    //             sep_value,
    //             Self::from_raw_parts(right_keys, right_values, right_len),
    //         )
    //     }
    // }

    #[inline]
    fn recalc_size(&mut self) {
        self.size = self.keys().iter().map(|key| key.size()).sum();
    }

    #[inline]
    pub fn split(&mut self, overflow_key: K, overflow_value: V, right: &mut Self) -> (K, V) {
        assert_eq!(self.len(), MAX_NUM_ELEMENTS);
        assert_eq!(right.len(), 0);

        unsafe {
            ptr::copy_nonoverlapping(
                self._elements.as_ptr().add(B + 1),
                right._elements.as_mut_ptr(),
                B - 2,
            );
            right.set_len(B - 2);

            right.recalc_size();
            self.size -= right.size();

            right.push(overflow_key, overflow_value).assert_none();

            self.set_len(B + 1);
            self.pop().unwrap()
        }
    }

    #[inline]
    pub fn merge(&mut self, sep_key: K, sep_value: V, right: &mut Self) {
        assert!(self.len() + right.len() < MAX_NUM_ELEMENTS);
        unsafe {
            self.push(sep_key, sep_value).assert_none();
            ptr::copy_nonoverlapping(
                right._elements.as_ptr(),
                self._elements.as_mut_ptr().add(self.len()),
                right.len(),
            );
            self.set_len(self.len() + right.len());
            right.set_len(0);

            self.size += right.size;
        }
    }
}

impl<K: OrdSize, V> Default for NodeElements<K, V> {
    fn default() -> Self {
        // Self::new(ptr::null_mut())
        Self::new()
    }
}

impl<K: OrdSize + Clone, V: Clone> Clone for NodeElements<K, V> {
    fn clone(&self) -> Self {
        unsafe {
            Self::from_raw_parts(
                self._keys.clone(self.len()).into_raw_parts().0,
                self._values.clone(self.len()).into_raw_parts().0,
                self.len(),
                self.size(),
                // self.parent,
            )
        }
    }
}

impl<K: OrdSize + fmt::Debug, V: fmt::Debug> fmt::Debug for NodeElements<K, V> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeData")
            .field("elements", &self.elements())
            .field("size", &self.size())
            .finish()
    }
}

impl<K: OrdSize, V> Drop for NodeElements<K, V> {
    fn drop(&mut self) {
        while let Some(_) = self.pop() {}
    }
}

struct Children<K: OrdSize, V> {
    _data: OuterLenChildren<K, V>,
    _len: usize,
}

enum ChildrenStackVec<K: OrdSize, V> {
    Nodes(StackVec<Box<Node<K, V>>, MAX_NUM_CHILDREN>),
    Leafs(StackVec<Box<NodeElements<K, V>>, MAX_NUM_CHILDREN>),
}

enum OuterLenChildren<K: OrdSize, V> {
    Nodes(OuterLenStackVec<Box<Node<K, V>>, MAX_NUM_CHILDREN>),
    Leafs(OuterLenStackVec<Box<NodeElements<K, V>>, MAX_NUM_CHILDREN>),
}

#[derive(Debug, Clone)]
enum Child<K: OrdSize, V> {
    Node(Box<Node<K, V>>),
    Leaf(Box<NodeElements<K, V>>),
}

#[derive(Debug)]
enum ChildRef<'a, K: OrdSize, V> {
    Node(&'a Node<K, V>),
    Leaf(&'a NodeElements<K, V>),
}

#[derive(Debug)]
enum ChildRefMut<'a, K: OrdSize, V> {
    Node(&'a mut Node<K, V>),
    Leaf(&'a mut NodeElements<K, V>),
}

#[derive(Debug)]
enum ChildrenSlice<'a, K: OrdSize, V> {
    Nodes(&'a [Box<Node<K, V>>]),
    Leafs(&'a [Box<NodeElements<K, V>>]),
}

#[derive(Debug)]
enum ChildrenSliceMut<'a, K: OrdSize, V> {
    Nodes(&'a mut [Box<Node<K, V>>]),
    Leafs(&'a mut [Box<NodeElements<K, V>>]),
}

#[derive(Debug)]
enum ChildrenIter<'a, K: OrdSize, V> {
    Nodes(std::slice::Iter<'a, Box<Node<K, V>>>),
    Leafs(std::slice::Iter<'a, Box<NodeElements<K, V>>>),
}

#[derive(Debug)]
enum ChildrenIterMut<'a, K: OrdSize, V> {
    Nodes(std::slice::IterMut<'a, Box<Node<K, V>>>),
    Leafs(std::slice::IterMut<'a, Box<NodeElements<K, V>>>),
}

#[derive(Debug, Clone)]
enum ChildrenIntoIter<K: OrdSize, V> {
    Nodes(StackVecIntoIter<Box<Node<K, V>>, MAX_NUM_CHILDREN>),
    Leafs(StackVecIntoIter<Box<NodeElements<K, V>>, MAX_NUM_CHILDREN>),
}

impl<K: OrdSize, V> Child<K, V> {
    pub fn num_elements(&self) -> usize {
        match self {
            Self::Node(node) => node.num_elements(),
            Self::Leaf(leaf) => leaf.len(),
        }
    }

    pub fn size(&self) -> usize {
        match self {
            Self::Node(node) => node.size(),
            Self::Leaf(leaf) => leaf.size(),
        }
    }

    // pub fn parent(&self) -> *const Node<T> {
    //     match self {
    //         Self::Node(node) => node.parent(),
    //         Self::Leaf(leaf) => leaf.parent,
    //     }
    // }

    // pub fn parent_mut(&mut self) -> &mut *mut Node<T> {
    //     match self {
    //         Self::Node(node) => node.parent_mut(),
    //         Self::Leaf(leaf) => &mut leaf.parent,
    //     }
    // }

    pub fn as_ref(&self) -> ChildRef<K, V> {
        match self {
            Self::Node(node) => ChildRef::Node(node),
            Self::Leaf(leaf) => ChildRef::Leaf(leaf),
        }
    }

    pub fn as_mut(&mut self) -> ChildRefMut<K, V> {
        match self {
            Self::Node(node) => ChildRefMut::Node(node),
            Self::Leaf(leaf) => ChildRefMut::Leaf(leaf),
        }
    }

    pub fn try_into_node(self) -> Option<Box<Node<K, V>>> {
        match self {
            Self::Node(node) => Some(node),
            Self::Leaf(_) => None,
        }
    }

    pub fn try_into_leaf(self) -> Option<Box<NodeElements<K, V>>> {
        match self {
            Self::Leaf(leaf) => Some(leaf),
            Self::Node(_) => None,
        }
    }

    pub fn try_as_node_ref(&self) -> Option<&Box<Node<K, V>>> {
        match self {
            Self::Node(node) => Some(node),
            Self::Leaf(_) => None,
        }
    }

    pub fn try_as_leaf_ref(&self) -> Option<&Box<NodeElements<K, V>>> {
        match self {
            Self::Leaf(leaf) => Some(leaf),
            Self::Node(_) => None,
        }
    }

    pub fn try_as_node_mut(&mut self) -> Option<&mut Box<Node<K, V>>> {
        match self {
            Self::Node(node) => Some(node),
            Self::Leaf(_) => None,
        }
    }

    pub fn try_as_leaf_mut(&mut self) -> Option<&mut Box<NodeElements<K, V>>> {
        match self {
            Self::Leaf(leaf) => Some(leaf),
            Self::Node(_) => None,
        }
    }

    pub fn keys(&self) -> &[K] {
        match self {
            Self::Node(node) => node.keys(),
            Self::Leaf(leaf) => leaf.keys(),
        }
    }

    pub fn keys_mut(&mut self) -> &mut [K] {
        match self {
            Self::Node(node) => node.keys_mut(),
            Self::Leaf(leaf) => leaf.keys_mut(),
        }
    }
    pub fn values(&self) -> &[V] {
        match self {
            Self::Node(node) => node.values(),
            Self::Leaf(leaf) => leaf.values(),
        }
    }

    pub fn values_mut(&mut self) -> &mut [V] {
        match self {
            Self::Node(node) => node.values_mut(),
            Self::Leaf(leaf) => leaf.values_mut(),
        }
    }

    #[inline]
    fn replace_with_child(&mut self) -> bool {
        assert_eq!(self.num_elements(), 0);

        match self {
            Self::Node(node) => unsafe {
                let mut child = node._children.pop(&mut 1).unwrap();
                // *child.parent_mut() = *node.parent_mut();
                mem::swap(self, &mut child);

                mem::forget::<Node<K, V>>(match child {
                    Self::Node(node) => *node,
                    Self::Leaf(_) => unreachable!(),
                });

                true
            },
            Self::Leaf(_) => false,
        }
    }
}

impl<'a, K: OrdSize, V> ChildRef<'a, K, V> {
    pub fn num_elements(&self) -> usize {
        match self {
            Self::Node(node) => node.num_elements(),
            Self::Leaf(leaf) => leaf.len(),
        }
    }

    pub fn size(&self) -> usize {
        match self {
            Self::Node(node) => node.size(),
            Self::Leaf(leaf) => leaf.size(),
        }
    }

    // pub fn parent(&self) -> *const Node<T> {
    //     match self {
    //         Self::Node(node) => node.parent(),
    //         Self::Leaf(leaf) => leaf.parent,
    //     }
    // }

    pub fn keys(&self) -> &'a [K] {
        match self {
            Self::Node(node) => node.keys(),
            Self::Leaf(leaf) => leaf.keys(),
        }
    }

    pub fn values(&self) -> &'a [V] {
        match self {
            Self::Node(node) => node.values(),
            Self::Leaf(leaf) => leaf.values(),
        }
    }

    pub fn try_into_node(self) -> Option<&'a Node<K, V>> {
        match self {
            Self::Node(node) => Some(node),
            Self::Leaf(_) => None,
        }
    }

    pub fn try_into_leaf(self) -> Option<&'a NodeElements<K, V>> {
        match self {
            Self::Leaf(leaf) => Some(leaf),
            Self::Node(_) => None,
        }
    }
}

impl<'a, K: OrdSize, V> Clone for ChildRef<'a, K, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, K: OrdSize, V> Copy for ChildRef<'a, K, V> {}

impl<'a, K: OrdSize, V> ChildRefMut<'a, K, V> {
    pub fn num_elements(&self) -> usize {
        match self {
            Self::Node(node) => node.num_elements(),
            Self::Leaf(leaf) => leaf.len(),
        }
    }

    pub fn size(&self) -> usize {
        match self {
            Self::Node(node) => node.size(),
            Self::Leaf(leaf) => leaf.size(),
        }
    }

    // pub fn parent(&self) -> *const Node<T> {
    //     match self {
    //         Self::Node(node) => node.parent(),
    //         Self::Leaf(leaf) => leaf.parent,
    //     }
    // }

    // pub fn parent_mut(&mut self) -> &mut *mut Node<T> {
    //     match self {
    //         Self::Node(node) => node.parent_mut(),
    //         Self::Leaf(leaf) => &mut leaf.parent,
    //     }
    // }

    pub fn as_ref(self) -> ChildRef<'a, K, V> {
        match self {
            Self::Node(node) => ChildRef::Node(node),
            Self::Leaf(leaf) => ChildRef::Leaf(leaf),
        }
    }

    pub fn borrow(&self) -> ChildRef<K, V> {
        match self {
            Self::Node(node) => ChildRef::Node(node),
            Self::Leaf(leaf) => ChildRef::Leaf(leaf),
        }
    }

    pub fn borrow_mut(&mut self) -> ChildRefMut<K, V> {
        match self {
            Self::Node(node) => ChildRefMut::Node(node),
            Self::Leaf(leaf) => ChildRefMut::Leaf(leaf),
        }
    }

    pub fn try_into_node(self) -> Option<&'a mut Node<K, V>> {
        match self {
            Self::Node(node) => Some(node),
            Self::Leaf(_) => None,
        }
    }

    pub fn try_into_leaf(self) -> Option<&'a mut NodeElements<K, V>> {
        match self {
            Self::Leaf(leaf) => Some(leaf),
            Self::Node(_) => None,
        }
    }

    pub fn keys(&self) -> &[K] {
        match self {
            Self::Node(node) => node.keys(),
            Self::Leaf(leaf) => leaf.keys(),
        }
    }

    pub fn keys_mut(&mut self) -> &mut [K] {
        match self {
            Self::Node(node) => node.keys_mut(),
            Self::Leaf(leaf) => leaf.keys_mut(),
        }
    }

    pub fn values(&self) -> &[V] {
        match self {
            Self::Node(node) => node.values(),
            Self::Leaf(leaf) => leaf.values(),
        }
    }

    pub fn values_mut(&mut self) -> &mut [V] {
        match self {
            Self::Node(node) => node.values_mut(),
            Self::Leaf(leaf) => leaf.values_mut(),
        }
    }

    pub fn swap(&mut self, other: ChildRefMut<K, V>) {
        match (self, other) {
            (Self::Node(self_node), ChildRefMut::Node(other_node)) => {
                mem::swap(*self_node, other_node)
            }
            (Self::Leaf(self_leaf), ChildRefMut::Leaf(other_leaf)) => {
                mem::swap(*self_leaf, other_leaf)
            }
            _ => panic!("called `ChildRefMut` where `self` and `other` are different (one is a leaf and the other is a node)"),
        }
    }
}

impl<'a, K: OrdSize, V> ChildrenSlice<'a, K, V> {
    pub fn get(&self, i: usize) -> Option<ChildRef<'a, K, V>> {
        match self {
            Self::Nodes(nodes) => Some(ChildRef::Node(nodes.get(i)?)),
            Self::Leafs(leafs) => Some(ChildRef::Leaf(leafs.get(i)?)),
        }
    }

    pub fn slice<B: std::ops::RangeBounds<usize>>(&self, bounds: B) -> Option<Self> {
        let bounds = (
            BoundClonedExt::cloned(&bounds.start_bound()),
            BoundClonedExt::cloned(&bounds.end_bound()),
        );

        match self {
            Self::Nodes(nodes) => Some(Self::Nodes(nodes.get(bounds)?)),
            Self::Leafs(leafs) => Some(Self::Leafs(leafs.get(bounds)?)),
        }
    }

    pub fn try_into_nodes(self) -> Option<&'a [Box<Node<K, V>>]> {
        match self {
            Self::Nodes(nodes) => Some(nodes),
            Self::Leafs(_) => None,
        }
    }

    pub fn try_into_leafs(self) -> Option<&'a [Box<NodeElements<K, V>>]> {
        match self {
            Self::Leafs(leafs) => Some(leafs),
            Self::Nodes(_) => None,
        }
    }

    #[inline]
    pub fn iter(&self) -> ChildrenIter<'a, K, V> {
        self.into_iter()
    }
}

impl<'a, K: OrdSize, V> Clone for ChildrenSlice<'a, K, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, K: OrdSize, V> Copy for ChildrenSlice<'a, K, V> {}

impl<'a, K: OrdSize, V> IntoIterator for ChildrenSlice<'a, K, V> {
    type Item = ChildRef<'a, K, V>;
    type IntoIter = ChildrenIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::Nodes(nodes) => ChildrenIter::Nodes(nodes.into_iter()),
            Self::Leafs(leafs) => ChildrenIter::Leafs(leafs.into_iter()),
        }
    }
}

impl<'a, K: OrdSize, V> ChildrenSliceMut<'a, K, V> {
    pub fn get(&self, i: usize) -> Option<ChildRef<K, V>> {
        match self {
            Self::Nodes(nodes) => Some(ChildRef::Node(nodes.get(i)?)),
            Self::Leafs(leafs) => Some(ChildRef::Leaf(leafs.get(i)?)),
        }
    }

    pub fn get_mut(&mut self, i: usize) -> Option<ChildRefMut<K, V>> {
        match self {
            Self::Nodes(nodes) => Some(ChildRefMut::Node(nodes.get_mut(i)?)),
            Self::Leafs(leafs) => Some(ChildRefMut::Leaf(leafs.get_mut(i)?)),
        }
    }

    pub fn drop_get(self, i: usize) -> Option<ChildRef<'a, K, V>> {
        match self {
            Self::Nodes(nodes) => Some(ChildRef::Node(nodes.get(i)?)),
            Self::Leafs(leafs) => Some(ChildRef::Leaf(leafs.get(i)?)),
        }
    }

    pub fn drop_get_mut(self, i: usize) -> Option<ChildRefMut<'a, K, V>> {
        match self {
            Self::Nodes(nodes) => Some(ChildRefMut::Node(nodes.get_mut(i)?)),
            Self::Leafs(leafs) => Some(ChildRefMut::Leaf(leafs.get_mut(i)?)),
        }
    }

    pub fn slice<B: std::ops::RangeBounds<usize>>(&self, bounds: B) -> Option<ChildrenSlice<K, V>> {
        let bounds = (
            BoundClonedExt::cloned(&bounds.start_bound()),
            BoundClonedExt::cloned(&bounds.end_bound()),
        );

        match self {
            Self::Nodes(nodes) => Some(ChildrenSlice::Nodes(nodes.get(bounds)?)),
            Self::Leafs(leafs) => Some(ChildrenSlice::Leafs(leafs.get(bounds)?)),
        }
    }

    pub fn slice_mut<B: std::ops::RangeBounds<usize>>(
        &mut self,
        bounds: B,
    ) -> Option<ChildrenSliceMut<K, V>> {
        let bounds = (
            BoundClonedExt::cloned(&bounds.start_bound()),
            BoundClonedExt::cloned(&bounds.end_bound()),
        );

        match self {
            Self::Nodes(nodes) => Some(ChildrenSliceMut::Nodes(nodes.get_mut(bounds)?)),
            Self::Leafs(leafs) => Some(ChildrenSliceMut::Leafs(leafs.get_mut(bounds)?)),
        }
    }

    pub fn drop_slice_mut<B: std::ops::RangeBounds<usize>>(self, bounds: B) -> Option<Self> {
        let bounds = (
            BoundClonedExt::cloned(&bounds.start_bound()),
            BoundClonedExt::cloned(&bounds.end_bound()),
        );

        match self {
            Self::Nodes(nodes) => Some(Self::Nodes(nodes.get_mut(bounds)?)),
            Self::Leafs(leafs) => Some(Self::Leafs(leafs.get_mut(bounds)?)),
        }
    }

    pub fn try_into_nodes(self) -> Option<&'a mut [Box<Node<K, V>>]> {
        match self {
            Self::Nodes(nodes) => Some(nodes),
            Self::Leafs(_) => None,
        }
    }

    pub fn try_into_leafs(self) -> Option<&'a mut [Box<NodeElements<K, V>>]> {
        match self {
            Self::Leafs(leafs) => Some(leafs),
            Self::Nodes(_) => None,
        }
    }

    pub fn iter(&self) -> ChildrenIter<K, V> {
        match self {
            Self::Nodes(nodes) => ChildrenIter::Nodes(nodes.iter()),
            Self::Leafs(leafs) => ChildrenIter::Leafs(leafs.iter()),
        }
    }

    pub fn iter_mut(&mut self) -> ChildrenIterMut<K, V> {
        match self {
            Self::Nodes(nodes) => ChildrenIterMut::Nodes(nodes.iter_mut()),
            Self::Leafs(leafs) => ChildrenIterMut::Leafs(leafs.iter_mut()),
        }
    }

    pub fn swap(&mut self, i: usize, j: usize) {
        match self {
            Self::Nodes(nodes) => {
                nodes.swap(i, j);
            }
            Self::Leafs(leafs) => {
                leafs.swap(i, j);
            }
        }
    }
}

impl<'a, K: OrdSize, V> IntoIterator for ChildrenSliceMut<'a, K, V> {
    type Item = ChildRefMut<'a, K, V>;
    type IntoIter = ChildrenIterMut<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::Nodes(nodes) => ChildrenIterMut::Nodes(nodes.into_iter()),
            Self::Leafs(leafs) => ChildrenIterMut::Leafs(leafs.into_iter()),
        }
    }
}

impl<'a, K: OrdSize, V> ExactSizeIterator for ChildrenIter<'a, K, V> {
    fn len(&self) -> usize {
        match self {
            Self::Nodes(nodes) => nodes.len(),
            Self::Leafs(leafs) => leafs.len(),
        }
    }
}

impl<'a, K: OrdSize, V> Iterator for ChildrenIter<'a, K, V> {
    type Item = ChildRef<'a, K, V>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Nodes(nodes_iter) => Some(ChildRef::Node(nodes_iter.next()?)),
            Self::Leafs(leafs_iter) => Some(ChildRef::Leaf(leafs_iter.next()?)),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Nodes(nodes) => nodes.size_hint(),
            Self::Leafs(leafs) => leafs.size_hint(),
        }
    }

    fn count(self) -> usize {
        match self {
            Self::Nodes(nodes) => nodes.count(),
            Self::Leafs(leafs) => leafs.count(),
        }
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        match self {
            Self::Nodes(nodes) => Some(ChildRef::Node(nodes.nth(n)?)),
            Self::Leafs(leafs) => Some(ChildRef::Leaf(leafs.nth(n)?)),
        }
    }

    #[inline]
    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }
}

impl<'a, K: OrdSize, V> DoubleEndedIterator for ChildrenIter<'a, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::Nodes(nodes_iter) => Some(ChildRef::Node(nodes_iter.next_back()?)),
            Self::Leafs(leafs_iter) => Some(ChildRef::Leaf(leafs_iter.next_back()?)),
        }
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        match self {
            Self::Nodes(nodes_iter) => Some(ChildRef::Node(nodes_iter.nth_back(n)?)),
            Self::Leafs(leafs_iter) => Some(ChildRef::Leaf(leafs_iter.nth_back(n)?)),
        }
    }
}

impl<'a, K: OrdSize, V> std::iter::FusedIterator for ChildrenIter<'a, K, V> {}

impl<'a, K: OrdSize, V> Clone for ChildrenIter<'a, K, V> {
    fn clone(&self) -> Self {
        match self {
            Self::Nodes(iter) => Self::Nodes(iter.clone()),
            Self::Leafs(iter) => Self::Leafs(iter.clone()),
        }
    }
}

impl<'a, K: OrdSize, V> ExactSizeIterator for ChildrenIterMut<'a, K, V> {
    fn len(&self) -> usize {
        match self {
            Self::Nodes(nodes) => nodes.len(),
            Self::Leafs(leafs) => leafs.len(),
        }
    }
}

impl<'a, K: OrdSize, V> Iterator for ChildrenIterMut<'a, K, V> {
    type Item = ChildRefMut<'a, K, V>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Nodes(nodes_iter) => Some(ChildRefMut::Node(nodes_iter.next()?)),
            Self::Leafs(leafs_iter) => Some(ChildRefMut::Leaf(leafs_iter.next()?)),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Nodes(nodes) => nodes.size_hint(),
            Self::Leafs(leafs) => leafs.size_hint(),
        }
    }

    fn count(self) -> usize {
        match self {
            Self::Nodes(nodes) => nodes.count(),
            Self::Leafs(leafs) => leafs.count(),
        }
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        match self {
            Self::Nodes(nodes) => Some(ChildRefMut::Node(nodes.nth(n)?)),
            Self::Leafs(leafs) => Some(ChildRefMut::Leaf(leafs.nth(n)?)),
        }
    }

    #[inline]
    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }
}

impl<'a, K: OrdSize, V> DoubleEndedIterator for ChildrenIterMut<'a, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::Nodes(nodes_iter) => Some(ChildRefMut::Node(nodes_iter.next_back()?)),
            Self::Leafs(leafs_iter) => Some(ChildRefMut::Leaf(leafs_iter.next_back()?)),
        }
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        match self {
            Self::Nodes(nodes_iter) => Some(ChildRefMut::Node(nodes_iter.nth_back(n)?)),
            Self::Leafs(leafs_iter) => Some(ChildRefMut::Leaf(leafs_iter.nth_back(n)?)),
        }
    }
}

impl<'a, K: OrdSize, V> std::iter::FusedIterator for ChildrenIterMut<'a, K, V> {}

impl<K: OrdSize, V> OuterLenChildren<K, V> {
    #[must_use]
    #[inline]
    pub fn new_nodes() -> Self {
        Self::Nodes(OuterLenStackVec::new())
    }

    #[must_use]
    #[inline]
    pub fn new_leafs() -> Self {
        Self::Leafs(OuterLenStackVec::new())
    }

    #[must_use]
    #[inline]
    pub unsafe fn push(&mut self, len: &mut usize, child: Child<K, V>) -> Option<Child<K, V>> {
        match (self, child) {
            (Self::Nodes(nodes), Child::Node(node)) => Some(Child::Node(nodes.push(len, node)?)),
            (Self::Leafs(leafs), Child::Leaf(leaf)) => Some(Child::Leaf(leafs.push(len, leaf)?)),
            _ => panic!("called `OuterLenChildren::push` where `self` and `child` are different (one is a leaf and the other is a node)"),
        }
    }

    #[must_use]
    #[inline]
    pub unsafe fn insert(
        &mut self,
        len: &mut usize,
        idx: usize,
        child: Child<K, V>,
    ) -> Option<Child<K, V>> {
        match (self, child) {
            (Self::Nodes(nodes), Child::Node(node)) => {
                Some(Child::Node(nodes.insert(len, idx, node)?))
            }
            (Self::Leafs(leafs), Child::Leaf(leaf)) => {
                Some(Child::Leaf(leafs.insert(len, idx, leaf)?))
            }
            _ => panic!("called `OuterLenChildren::insert` where `self` and `child` are different (one is a leaf and the other is a node)"),
        }
    }

    #[inline]
    pub unsafe fn pop(&mut self, len: &mut usize) -> Option<Child<K, V>> {
        match self {
            Self::Nodes(nodes) => Some(Child::Node(nodes.pop(len)?)),
            Self::Leafs(leafs) => Some(Child::Leaf(leafs.pop(len)?)),
        }
    }

    #[inline]
    pub unsafe fn remove(&mut self, len: &mut usize, idx: usize) -> Child<K, V> {
        match self {
            Self::Nodes(nodes) => Child::Node(nodes.remove(len, idx)),
            Self::Leafs(leafs) => Child::Leaf(leafs.remove(len, idx)),
        }
    }

    #[inline]
    pub unsafe fn split_at(&mut self, len: &mut usize, left_len: usize) -> Children<K, V> {
        match self {
            Self::Nodes(nodes) => {
                let (right_nodes, right_len) = nodes.split_at(len, left_len).into_raw_parts();
                Children::from_raw_parts(Self::Nodes(right_nodes), right_len)
            }
            Self::Leafs(leafs) => {
                let (right_leafs, right_len) = leafs.split_at(len, left_len).into_raw_parts();
                Children::from_raw_parts(Self::Leafs(right_leafs), right_len)
            }
        }
    }

    #[inline]
    pub unsafe fn as_slice(&self, len: usize) -> ChildrenSlice<K, V> {
        match self {
            Self::Nodes(nodes) => ChildrenSlice::Nodes(nodes.as_slice(len)),
            Self::Leafs(leafs) => ChildrenSlice::Leafs(leafs.as_slice(len)),
        }
    }

    #[inline]
    pub unsafe fn as_slice_mut(&mut self, len: usize) -> ChildrenSliceMut<K, V> {
        match self {
            Self::Nodes(nodes) => ChildrenSliceMut::Nodes(nodes.as_slice_mut(len)),
            Self::Leafs(leafs) => ChildrenSliceMut::Leafs(leafs.as_slice_mut(len)),
        }
    }

    #[inline]
    pub unsafe fn clone(&self, len: usize) -> Children<K, V>
    where
        K: Clone,
        V: Clone,
    {
        match self {
            Self::Nodes(nodes) => {
                let (nodes, len) = nodes.clone(len).into_raw_parts();
                Children::from_raw_parts(Self::Nodes(nodes), len)
            }
            Self::Leafs(leafs) => {
                let (leafs, len) = leafs.clone(len).into_raw_parts();
                Children::from_raw_parts(Self::Leafs(leafs), len)
            }
        }
    }
}

impl<K: OrdSize, V> Children<K, V> {
    #[must_use]
    #[inline]
    pub fn new_nodes() -> Self {
        Self {
            _data: OuterLenChildren::Nodes(OuterLenStackVec::new()),
            _len: 0,
        }
    }

    #[must_use]
    #[inline]
    pub fn new_leafs() -> Self {
        Self {
            _data: OuterLenChildren::Leafs(OuterLenStackVec::new()),
            _len: 0,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self._len
    }

    #[must_use]
    pub fn push(&mut self, child: Child<K, V>) -> Option<Child<K, V>> {
        match (&mut self._data, child) {
            (OuterLenChildren::Nodes(nodes), Child::Node(node)) => Some(Child::Node(unsafe { nodes.push(&mut self._len, node)? })),
            (OuterLenChildren::Leafs(leafs), Child::Leaf(leaf)) => Some(Child::Leaf(unsafe { leafs.push(&mut self._len, leaf)? })),
            _ => panic!("called `Children::push` where `self` and `child` are different (one is a leaf and the other is a node)"),
        }
    }

    #[must_use]
    pub fn insert(&mut self, idx: usize, child: Child<K, V>) -> Option<Child<K, V>> {
        match (&mut self._data, child) {
            (OuterLenChildren::Nodes(nodes), Child::Node(node)) => {
                Some(Child::Node(unsafe { nodes.insert(&mut self._len, idx, node)? }))
            }
            (OuterLenChildren::Leafs(leafs), Child::Leaf(leaf)) => {
                Some(Child::Leaf(unsafe { leafs.insert(&mut self._len, idx, leaf)? }))
            }
            _ => panic!("called `Children::insert` where `self` and `child` are different (one is a leaf and the other is a node)"),
        }
    }

    pub fn pop(&mut self) -> Option<Child<K, V>> {
        match &mut self._data {
            OuterLenChildren::Nodes(nodes) => {
                Some(Child::Node(unsafe { nodes.pop(&mut self._len)? }))
            }
            OuterLenChildren::Leafs(leafs) => {
                Some(Child::Leaf(unsafe { leafs.pop(&mut self._len)? }))
            }
        }
    }

    pub fn remove(&mut self, idx: usize) -> Child<K, V> {
        match &mut self._data {
            OuterLenChildren::Nodes(nodes) => {
                Child::Node(unsafe { nodes.remove(&mut self._len, idx) })
            }
            OuterLenChildren::Leafs(leafs) => {
                Child::Leaf(unsafe { leafs.remove(&mut self._len, idx) })
            }
        }
    }

    pub fn split_at(&mut self, left_len: usize) -> Self {
        match &mut self._data {
            OuterLenChildren::Nodes(nodes) => unsafe {
                let (right_data, right_len) =
                    nodes.split_at(&mut self._len, left_len).into_raw_parts();
                Self::from_raw_parts(OuterLenChildren::Nodes(right_data), right_len)
            },
            OuterLenChildren::Leafs(leafs) => unsafe {
                let (right_data, right_len) =
                    leafs.split_at(&mut self._len, left_len).into_raw_parts();
                Self::from_raw_parts(OuterLenChildren::Leafs(right_data), right_len)
            },
        }
    }

    #[inline]
    pub fn as_slice(&self) -> ChildrenSlice<K, V> {
        match &self._data {
            OuterLenChildren::Nodes(nodes) => {
                ChildrenSlice::Nodes(unsafe { nodes.as_slice(self.len()) })
            }
            OuterLenChildren::Leafs(leafs) => {
                ChildrenSlice::Leafs(unsafe { leafs.as_slice(self.len()) })
            }
        }
    }

    #[inline]
    pub fn as_slice_mut(&mut self) -> ChildrenSliceMut<K, V> {
        let len = self.len();
        match &mut self._data {
            OuterLenChildren::Nodes(nodes) => {
                ChildrenSliceMut::Nodes(unsafe { nodes.as_slice_mut(len) })
            }
            OuterLenChildren::Leafs(leafs) => {
                ChildrenSliceMut::Leafs(unsafe { leafs.as_slice_mut(len) })
            }
        }
    }

    #[inline]
    unsafe fn from_raw_parts(children: OuterLenChildren<K, V>, len: usize) -> Self {
        Self {
            _data: children,
            _len: len,
        }
    }

    #[inline]
    fn into_raw_parts(self) -> (OuterLenChildren<K, V>, usize) {
        unsafe {
            let mb = mem::ManuallyDrop::new(self);
            (ptr::read(&mb._data), mb._len)
        }
    }
}

impl<K: OrdSize, V> IntoIterator for Children<K, V> {
    type Item = Child<K, V>;
    type IntoIter = ChildrenIntoIter<K, V>;

    fn into_iter(self) -> ChildrenIntoIter<K, V> {
        unsafe {
            let (data, len) = self.into_raw_parts();
            match data {
                OuterLenChildren::Nodes(data) => {
                    ChildrenIntoIter::Nodes(StackVec::from_raw_parts(data, len).into_iter())
                }
                OuterLenChildren::Leafs(data) => {
                    ChildrenIntoIter::Leafs(StackVec::from_raw_parts(data, len).into_iter())
                }
            }
        }
    }
}

impl<K: OrdSize, V> Drop for Children<K, V> {
    fn drop(&mut self) {
        while let Some(_) = self.pop() {}
    }
}

impl<K: OrdSize + Clone, V: Clone> Clone for Children<K, V> {
    fn clone(&self) -> Self {
        unsafe { self._data.clone(self._len) }
    }
}

impl<K: OrdSize + fmt::Debug, V: fmt::Debug> fmt::Debug for Children<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_slice().fmt(f)
    }
}

impl<K: OrdSize, V> Into<ChildrenStackVec<K, V>> for Children<K, V> {
    #[inline]
    fn into(self) -> ChildrenStackVec<K, V> {
        unsafe {
            let (data, len) = self.into_raw_parts();

            match data {
                OuterLenChildren::Nodes(nodes) => {
                    ChildrenStackVec::Nodes(StackVec::from_raw_parts(nodes, len))
                }
                OuterLenChildren::Leafs(leafs) => {
                    ChildrenStackVec::Leafs(StackVec::from_raw_parts(leafs, len))
                }
            }
        }
    }
}

impl<K: OrdSize, V> Into<Children<K, V>> for ChildrenStackVec<K, V> {
    #[inline]
    fn into(self) -> Children<K, V> {
        unsafe {
            match self {
                ChildrenStackVec::Nodes(nodes) => {
                    let (data, len) = nodes.into_raw_parts();
                    Children::from_raw_parts(OuterLenChildren::Nodes(data), len)
                }
                ChildrenStackVec::Leafs(leafs) => {
                    let (data, len) = leafs.into_raw_parts();
                    Children::from_raw_parts(OuterLenChildren::Leafs(data), len)
                }
            }
        }
    }
}

impl<K: OrdSize, V> Iterator for ChildrenIntoIter<K, V> {
    type Item = Child<K, V>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Nodes(nodes) => Some(Child::Node(nodes.next()?)),
            Self::Leafs(leafs) => Some(Child::Leaf(leafs.next()?)),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Nodes(nodes) => nodes.size_hint(),
            Self::Leafs(leafs) => leafs.size_hint(),
        }
    }

    fn count(self) -> usize {
        match self {
            Self::Nodes(nodes) => nodes.count(),
            Self::Leafs(leafs) => leafs.count(),
        }
    }
}

impl<K: OrdSize, V> DoubleEndedIterator for ChildrenIntoIter<K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::Nodes(nodes) => Some(Child::Node(nodes.next_back()?)),
            Self::Leafs(leafs) => Some(Child::Leaf(leafs.next_back()?)),
        }
    }
}

impl<K: OrdSize, V> ExactSizeIterator for ChildrenIntoIter<K, V> {
    #[inline]
    fn len(&self) -> usize {
        match self {
            Self::Nodes(nodes) => nodes.len(),
            Self::Leafs(leafs) => leafs.len(),
        }
    }
}

struct Node<K: OrdSize, V> {
    _elements: NodeElements<K, V>,
    _children: OuterLenChildren<K, V>,
}

impl<K: OrdSize, V> Node<K, V> {
    // pub fn new(child: Child<T>, parent: *mut Self) -> Box<Self> {
    pub fn new(child: Child<K, V>) -> Box<Self> {
        unsafe {
            match child {
                Child::Node(node) => {
                    let mut slf = Box::new(Self::from_raw_parts(
                        // NodeElements::new(parent),
                        NodeElements::new(),
                        OuterLenChildren::Nodes(OuterLenStackVec::new()),
                    ));
                    // *node.parent_mut() = slf.as_mut();
                    slf.set_size(node.size());
                    slf._children.push(&mut 0, Child::Node(node)).assert_none();
                    slf
                }
                Child::Leaf(leaf) => {
                    let mut slf = Box::new(Self::from_raw_parts(
                        // NodeElements::new(parent),
                        NodeElements::new(),
                        OuterLenChildren::Leafs(OuterLenStackVec::new()),
                    ));
                    // leaf.parent = slf.as_mut();
                    slf.set_size(leaf.size());
                    slf._children.push(&mut 0, Child::Leaf(leaf)).assert_none();
                    slf
                }
            }
        }
    }

    #[inline]
    pub fn keys(&self) -> &[K] {
        self._elements.keys()
    }

    #[inline]
    pub fn keys_mut(&mut self) -> &mut [K] {
        self._elements.keys_mut()
    }

    #[inline]
    pub fn values(&self) -> &[V] {
        self._elements.keys()
    }

    #[inline]
    pub fn values_mut(&mut self) -> &mut [V] {
        self._elements.keys_mut()
    }

    #[inline]
    pub fn children(&self) -> ChildrenSlice<K, V> {
        unsafe { self._children.as_slice(self.num_children()) }
    }

    #[inline]
    pub fn children_mut(&mut self) -> ChildrenSliceMut<K, V> {
        unsafe { self._children.as_slice_mut(self.num_children()) }
    }

    #[inline]
    pub fn get_all_mut(&mut self) -> (&mut [K], &mut [V], ChildrenSliceMut<K, V>) {
        let children = unsafe { self._children.as_slice_mut(self.num_children()) };
        let (keys, values) = self._elements.get_all_mut();
        (keys, values, children)
    }

    #[must_use]
    pub fn push(&mut self, key: K, value: V, child: Child<K, V>) -> Option<(K, V, Child<K, V>)> {
        unsafe {
            let mut old_num_children = self.num_children();

            let overflow_element = self._elements.push(key, value);

            // *child.parent_mut() = self;
            self._elements.size += child.size();
            let overflow_child = self._children.push(&mut old_num_children, child);

            match (overflow_element, overflow_child) {
                (Some((overflow_key, overflow_value)), Some(overflow_child)) => {
                    self._elements.size -= overflow_child.size();

                    Some((overflow_key, overflow_value, overflow_child))
                }
                (None, None) => None,
                _ => unreachable!(),
            }
        }
    }

    #[must_use]
    pub fn insert(
        &mut self,
        idx: usize,
        key: K,
        value: V,
        child: Child<K, V>,
    ) -> Option<(K, V, Child<K, V>)> {
        unsafe {
            let mut old_num_children = self.num_children();

            let overflow_element = self._elements.insert(idx, key, value);

            // *child.parent_mut() = self;
            self._elements.size += child.size();
            let overflow_child = self._children.insert(&mut old_num_children, idx + 1, child);

            match (overflow_element, overflow_child) {
                (Some((overflow_key, overflow_value)), Some(overflow_child)) => {
                    self._elements.size -= overflow_child.size();

                    Some((overflow_key, overflow_value, overflow_child))
                }
                (None, None) => None,
                _ => unreachable!(),
            }
        }
    }

    pub fn pop(&mut self) -> Option<(K, V, Child<K, V>)> {
        unsafe {
            let mut old_num_children = self.num_children();

            let popped_element = self._elements.pop();

            if let Some((popped_key, popped_value)) = popped_element {
                let popped_child = self._children.pop(&mut old_num_children).unwrap();
                self._elements.size -= popped_child.size();

                Some((popped_key, popped_value, popped_child))
            } else {
                None
            }
        }
    }

    pub fn remove(&mut self, idx: usize) -> (K, V, Child<K, V>) {
        unsafe {
            let mut old_num_children = self.num_children();

            let (removed_key, removed_value) = self._elements.remove(idx);
            let removed_child = self._children.remove(&mut old_num_children, idx + 1);

            self._elements.size -= removed_child.size();

            (removed_key, removed_value, removed_child)
        }
    }

    // pub fn split(
    //     &mut self,
    //     rightmost_k: K,
    //     rightmost_value: V,
    //     rightmost_child: Child<T>,
    // ) -> (K, V, Box<Self>) {
    //     unsafe {
    //         let mut num_children = self.num_children();

    //         let (sep_k, sep_value, right_elements) =
    //             self._elements.split(rightmost_k, rightmost_value);
    //         let mut right_children = self
    //             ._children
    //             .split_at(&mut num_children, self.num_children())
    //             .into_raw_parts()
    //             .0;
    //         right_children
    //             .push(&mut right_elements.len(), rightmost_child)
    //             .assert_none();

    //         //      [kv0| kv1 |kv2] kvr
    //         // [ch0, ch1||ch2, ch3] chr
    //         let right = Box::new(Self::from_raw_parts(right_elements, right_children));

    //         // let right_mut_ptr = right.as_mut() as *mut Self;
    //         // match right.children_mut() {
    //         //     ChildrenSliceMut::Nodes(nodes) => {
    //         //         for node in nodes {
    //         //             *node.parent_mut() = right_mut_ptr;
    //         //         }
    //         //     }
    //         //     ChildrenSliceMut::Leafs(leafs) => {
    //         //         for leaf in leafs {
    //         //             leaf.parent = right_mut_ptr;
    //         //         }
    //         //     }
    //         // }

    //         (sep_k, sep_value, right)
    //     }
    // }

    pub fn split(
        &mut self,
        overflow_key: K,
        overflow_value: V,
        overflow_child: Child<K, V>,
    ) -> (K, V, Box<Self>) {
        let mut right = Box::new(Self {
            _elements: NodeElements::new(),
            _children: match self._children {
                OuterLenChildren::Nodes(_) => OuterLenChildren::Nodes(OuterLenStackVec::new()),
                OuterLenChildren::Leafs(_) => OuterLenChildren::Leafs(OuterLenStackVec::new()),
            },
        });
        unsafe {
            let sep_element =
                self._elements
                    .split(overflow_key, overflow_value, &mut right._elements);

            match (&self._children, &mut right._children, overflow_child) {
                (
                    OuterLenChildren::Nodes(self_children),
                    OuterLenChildren::Nodes(right_children),
                    Child::Node(rightmost_child),
                ) => {
                    ptr::copy_nonoverlapping(
                        self_children.as_ptr().add(B + 1),
                        right_children.as_mut_ptr(),
                        B - 1,
                    );

                    let mut right_children_size: usize = right_children
                        .as_slice(B - 1)
                        .iter()
                        .map(|child| child.size())
                        .sum();
                    self.set_size(self.size() - right_children_size);

                    right_children_size += rightmost_child.size();
                    right_children
                        .push(&mut (B - 1), rightmost_child)
                        .assert_none();

                    right.set_size(right.size() + right_children_size);
                }
                (
                    OuterLenChildren::Leafs(self_children),
                    OuterLenChildren::Leafs(right_children),
                    Child::Leaf(rightmost_child),
                ) => {
                    ptr::copy_nonoverlapping(
                        self_children.as_ptr().add(B + 1),
                        right_children.as_mut_ptr(),
                        B - 1,
                    );

                    let mut right_children_size: usize = right_children
                        .as_slice(B - 1)
                        .iter()
                        .map(|child| child.size())
                        .sum();
                    self.set_size(self.size() - right_children_size);

                    right_children_size += rightmost_child.size();
                    right_children
                        .push(&mut (B - 1), rightmost_child)
                        .assert_none();

                    right.set_size(right.size() + right_children_size);
                }
                _ => unreachable!(),
            }

            (sep_element, right)
        }
    }

    #[inline]
    pub fn merge(&mut self, sep_key: K, sep_value: V, mut right: Box<Self>) {
        assert!(self.num_elements() + right.num_elements() < MAX_NUM_ELEMENTS);
        unsafe {
            match (&mut self._children, &right._children) {
                (
                    OuterLenChildren::Nodes(self_children),
                    OuterLenChildren::Nodes(right_children),
                ) => {
                    ptr::copy_nonoverlapping(
                        right_children.as_ptr(),
                        self_children.as_mut_ptr().add(self.num_children()),
                        right.num_children(),
                    );
                }
                (
                    OuterLenChildren::Leafs(self_children),
                    OuterLenChildren::Leafs(right_children),
                ) => {
                    ptr::copy_nonoverlapping(
                        right_children.as_ptr(),
                        self_children.as_mut_ptr().add(self.num_children()),
                        right.num_children(),
                    );
                }
                _ => unreachable!(),
            }
            self._elements
                .merge(sep_key, sep_value, &mut right._elements);
            mem::forget(*right);
        }
    }

    fn into_raw_parts(self) -> (NodeElements<K, V>, OuterLenChildren<K, V>) {
        unsafe {
            let mb = mem::ManuallyDrop::new(self);
            (ptr::read(&mb._elements), ptr::read(&mb._children))
        }
    }

    unsafe fn from_raw_parts(
        elements: NodeElements<K, V>,
        children: OuterLenChildren<K, V>,
    ) -> Self {
        Self {
            _elements: elements,
            _children: children,
        }
    }

    pub fn separate(self) -> (NodeElements<K, V>, Children<K, V>) {
        unsafe {
            let num_children = self.num_children();

            let (elements, children) = self.into_raw_parts();

            (elements, Children::from_raw_parts(children, num_children))
        }
    }

    #[inline(always)]
    pub fn num_children(&self) -> usize {
        self.num_elements() + 1
    }
    #[inline(always)]
    pub fn num_elements(&self) -> usize {
        self._elements.len()
    }

    #[inline(always)]
    pub fn size(&self) -> usize {
        self._elements.size()
    }

    // #[inline(always)]
    // pub fn parent(&self) -> *const Self {
    //     self._elements.parent
    // }

    // #[inline(always)]
    // pub fn parent_mut(&mut self) -> &mut *mut Self {
    //     &mut self._elements.parent
    // }

    #[inline(always)]
    pub unsafe fn set_num_elements(&mut self, num_elements: usize) {
        self._elements.set_len(num_elements);
    }

    #[inline(always)]
    pub unsafe fn set_size(&mut self, size: usize) {
        self._elements.set_size(size);
    }
}

impl<K: OrdSize + Clone, V> Clone for Node<K, V> {
    fn clone(&self) -> Self {
        unsafe {
            Self::from_raw_parts(
                self._elements.clone(),
                self._children.clone(self.num_children()).into_raw_parts().0,
            )
        }
    }
}

impl<K: OrdSize + fmt::Debug, V> fmt::Debug for Node<K, V> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            // .field("elements", &self._elements)
            .field("elements", &self.elements())
            .field("children", &self.children())
            .field("size", &self.size())
            .finish()
    }
}

impl<K: OrdSize, V> Drop for Node<K, V> {
    fn drop(&mut self) {
        while let Some(_) = self.pop() {}
        unsafe {
            self._children.pop(&mut 1).unwrap();
        }
    }
}

// #[inline]
// pub fn lin_search<T, Q>(a: &[T], k: &Q) -> Result<usize, usize>
// where
//     T: Ord + Borrow<Q>,
//     Q: Ord + ?Sized,
// {
//     lin_search_by(a, |x| x.borrow().cmp(k))
// }
//
// #[inline]
// pub fn lin_search_by<T, F: FnMut(&T) -> Ordering>(a: &[T], mut f: F) -> Result<usize, usize> {
//     // const LIN_SEARCH_SIZE: usize = 20;
//
//     // let mut size = a.len();
//     // let mut left = 0;
//     // let mut right = size;
//
//     // while LIN_SEARCH_SIZE < size {
//     //     let mid = left + size / 2;
//
//     //     let cmp = f(unsafe { a.get_unchecked(mid) });
//
//     //     if cmp == Ordering::Less {
//     //         left = mid + 1;
//     //     } else if cmp == Ordering::Greater {
//     //         right = mid;
//     //     } else {
//     //         return Ok(mid);
//     //     }
//     //     size = right - left;
//     // }
//
//     for (i, x) in a.iter().enumerate() {
//         let cmp = f(x);
//         if cmp == Ordering::Greater {
//             return Err(i);
//         } else if cmp == Ordering::Equal {
//             return Ok(i);
//         }
//     }
//     Err(a.len())
// }

#[derive(Debug, Clone)]
pub struct OrdBTree<K: OrdSize, V> {
    root: Child<K, V>,
    len: usize,
    depth: usize,
}

// pub struct RefMutBTreeElement<'a, K: OrdSize, V>(RefMutBTreeElementInner<'a, K, V>);
// enum RefMutBTreeElementInner<'a, K: OrdSize, V> {
//     NodeRootElemOnLeaf {
//         ref_stack: RefMutStack<'a, Node<K, V>>,
//         leaf_idx: usize,
//         elem_idx: usize,
//         prev_size: usize,
//     },
//     NodeRootElemOnNode {
//         ref_stack: RefMutStack<'a, Node<K, V>>,
//         elem_idx: usize,
//         prev_size: usize,
//     },
//     LeafRoot {
//         root: &'a mut NodeElements<K, V>,
//         elem_idx: usize,
//         prev_size: usize,
//     },
// }
//
// impl<'a, K: OrdSize, V> Deref for RefMutBTreeElementInner<'a, K, V> {
//     type Target = K;
//
//     fn deref(&self) -> &K {
//         match *self {
//             Self::NodeRootElemOnLeaf {
//                 ref ref_stack,
//                 leaf_idx,
//                 elem_idx,
//                 ..
//             } => {
//                 let node = ref_stack.peek().unwrap();
//                 let leaf = &node.children().try_into_leafs().unwrap()[leaf_idx];
//
//                 &leaf.elements()[elem_idx]
//             }
//             Self::NodeRootElemOnNode {
//                 ref ref_stack,
//                 elem_idx,
//                 ..
//             } => {
//                 let node = ref_stack.peek().unwrap();
//                 &node.elements()[elem_idx]
//             }
//             Self::LeafRoot {
//                 ref root, elem_idx, ..
//             } => &root.elements()[elem_idx],
//         }
//     }
// }
//
// impl<'a, T: OrdSize> DerefMut for RefMutBTreeElementInner<'a, T> {
//     fn deref_mut(&mut self) -> &mut T {
//         // let node = self.ref_stack.peek_mut().unwrap();
//         // let leaf = node.children_mut().try_into_leafs().unwrap()[self.leaf_idx];
//
//         // &mut leaf.elements_mut()[self.elem_idx]
//         match *self {
//             Self::NodeRootElemOnLeaf {
//                 ref mut ref_stack,
//                 leaf_idx,
//                 elem_idx,
//                 ..
//             } => {
//                 let node = ref_stack.peek_mut().unwrap();
//                 let leaf = &mut node.children_mut().try_into_leafs().unwrap()[leaf_idx];
//
//                 &mut leaf.elements_mut()[elem_idx]
//             }
//             Self::NodeRootElemOnNode {
//                 ref mut ref_stack,
//                 elem_idx,
//                 ..
//             } => {
//                 let node = ref_stack.peek_mut().unwrap();
//                 &mut node.elements_mut()[elem_idx]
//             }
//             Self::LeafRoot {
//                 ref mut root,
//                 elem_idx,
//                 ..
//             } => &mut root.elements_mut()[elem_idx],
//         }
//     }
// }
//
// impl<'a, T: OrdSize> Drop for RefMutBTreeElementInner<'a, T> {
//     fn drop(&mut self) {
//         match *self {
//             Self::NodeRootElemOnLeaf {
//                 ref mut ref_stack,
//                 leaf_idx,
//                 elem_idx,
//                 prev_size,
//             } => {
//                 let node = ref_stack.peek_mut().unwrap();
//                 let leaf = &mut node.children_mut().try_into_leafs().unwrap()[leaf_idx];
//                 let elem_size = leaf.elements()[elem_idx].size();
//                 assert!(0 < elem_size);
//
//                 let diff = elem_size as isize - prev_size as isize;
//
//                 if diff != 0 {
//                     unsafe {
//                         leaf.set_size((leaf.size() as isize + diff) as _);
//                     }
//
//                     loop {
//                         let node = ref_stack.peek_mut().unwrap();
//                         unsafe {
//                             node.set_size((node.size() as isize + diff) as _);
//                         }
//                         if let Some(_root) = node.pop() {
//                             break;
//                         }
//                     }
//                 }
//             }
//             Self::NodeRootElemOnNode {
//                 ref mut ref_stack,
//                 elem_idx,
//                 prev_size,
//             } => {
//                 let node = ref_stack.peek().unwrap();
//                 let elem_size = node.elements()[elem_idx].size();
//                 assert!(0 < elem_size);
//
//                 let diff = elem_size as isize - prev_size as isize;
//
//                 if diff != 0 {
//                     loop {
//                         let node = ref_stack.peek_mut().unwrap();
//                         unsafe {
//                             node.set_size((node.size() as isize + diff) as _);
//                         }
//                         if let Some(_root) = node.pop() {
//                             break;
//                         }
//                     }
//                 }
//             }
//             Self::LeafRoot {
//                 ref mut root,
//                 elem_idx,
//                 prev_size,
//             } => {
//                 let elem_size = root.elements()[elem_idx].size();
//                 assert!(0 < elem_size);
//
//                 let diff = elem_size as isize - prev_size as isize;
//                 unsafe {
//                     root.set_size((root.size() as isize + diff) as _);
//                 }
//             }
//         }
//     }
// }

impl<K: OrdSize, V> OrdBTree<K, V> {
    pub fn new() -> Self {
        Self {
            // root: Child::Leaf(Box::new(NodeElements::new(ptr::null_mut()))),
            root: Child::Leaf(Box::new(NodeElements::new())),
            len: 0,
            depth: 1,
        }
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.root.size()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn depth(&self) -> usize {
        self.depth
    }

    pub fn get(&self, key: usize) -> Option<(usize, &K, &V)> {
        let mut child = self.root.as_ref();
        let mut partial_sum = 0;

        'get_loop: loop {
            match child {
                ChildRef::Node(node) => match node.children() {
                    ChildrenSlice::Nodes(children) => {
                        for (elem, iter_child) in node.elements().iter().zip(children.iter()) {
                            let iter_child = iter_child.as_ref();

                            let child_size = iter_child.size();
                            partial_sum += child_size;
                            if key < partial_sum {
                                partial_sum -= child_size;
                                child = ChildRef::Node(iter_child);
                                continue 'get_loop;
                            }

                            let elem_size = elem.size();
                            partial_sum += elem_size;
                            if key < partial_sum {
                                return Some((partial_sum - elem_size, elem));
                            }
                        }
                        child = ChildRef::Node(children.last().unwrap());
                    }
                    ChildrenSlice::Leafs(children) => {
                        for (elem, iter_child) in node.elements().iter().zip(children.iter()) {
                            let iter_child = iter_child.as_ref();

                            let child_size = iter_child.size();
                            partial_sum += child_size;
                            if key < partial_sum {
                                partial_sum -= child_size;
                                child = ChildRef::Leaf(iter_child);
                                continue 'get_loop;
                            }

                            let elem_size = elem.size();
                            partial_sum += elem_size;
                            if key < partial_sum {
                                return Some((partial_sum - elem_size, elem));
                            }
                        }
                        child = ChildRef::Leaf(children.last().unwrap());
                    } // for (i, elem) in node.elements().iter().enumerate() {
                      //     let child_size = node.children().get(i).unwrap().size();
                      //     partial_sum += child_size;
                      //     if key < partial_sum {
                      //         partial_sum -= child_size;
                      //         child = node.children().get(i).unwrap();
                      //         continue 'get_loop;
                      //     }

                      //     let elem_size = elem.size();
                      //     partial_sum += elem_size;
                      //     if key < partial_sum {
                      //         return Some((partial_sum - elem_size, elem));
                      //     }
                      // }
                      // let last_child_idx = node.num_elements();
                      // child = node.children().get(last_child_idx).unwrap();
                },
                ChildRef::Leaf(leaf) => {
                    for elem in leaf.elements().iter() {
                        let elem_size = elem.size();
                        partial_sum += elem_size;
                        if key < partial_sum {
                            return Some((partial_sum - elem_size, elem));
                        }
                    }
                    return None;
                }
            }
        }
    }

    // pub fn get_mut(&mut self, key: usize) -> Option<(usize, RefMutBTreeElement<K, V>)> {
    //     match &mut self.root {
    //         Child::Node(root) => {
    //             let mut partial_sum = 0;
    //             let mut ref_stack = RefMutStack::with_capacity(20);
    //             ref_stack.push_root(root.as_mut());

    //             // 'get_loop:
    //             loop {
    //                 let node = ref_stack.peek().unwrap();

    //                 let mut child_idx = node.num_elements();
    //                 for (i, elem) in node.elements().iter().enumerate() {
    //                     let child_size = node.children().get(i).unwrap().size();
    //                     partial_sum += child_size;
    //                     if key < partial_sum {
    //                         partial_sum -= child_size;
    //                         child_idx = i;
    //                         break;
    //                     }

    //                     let elem_size = elem.size();
    //                     partial_sum += elem_size;
    //                     if key < partial_sum {
    //                         return Some((
    //                             partial_sum - elem_size,
    //                             RefMutBTreeElement(RefMutBTreeElementInner::NodeRootElemOnNode {
    //                                 ref_stack,
    //                                 elem_idx: i,
    //                                 prev_size: elem_size,
    //                             }),
    //                         ));
    //                     }
    //                 }

    //                 match ref_stack.try_push(|node| match node.children_mut() {
    //                     ChildrenSliceMut::Nodes(children_nodes) => {
    //                         Ok(children_nodes[child_idx].as_mut())
    //                     }
    //                     ChildrenSliceMut::Leafs(leafs) => Err(leafs[child_idx].as_mut()),
    //                 }) {
    //                     Ok(successfully_pushed) => assert!(successfully_pushed),
    //                     Err(leaf) => {
    //                         for (i, elem) in leaf.elements().iter().enumerate() {
    //                             let elem_size = elem.size();
    //                             partial_sum += elem_size;
    //                             if key < partial_sum {
    //                                 return Some((
    //                                     partial_sum - elem_size,
    //                                     RefMutBTreeElement(
    //                                         RefMutBTreeElementInner::NodeRootElemOnLeaf {
    //                                             ref_stack,
    //                                             leaf_idx: child_idx,
    //                                             elem_idx: i,
    //                                             prev_size: elem_size,
    //                                         },
    //                                     ),
    //                                 ));
    //                             }
    //                         }
    //                         return None;
    //                     }
    //                 }
    //             }
    //         }
    //         Child::Leaf(root) => {
    //             let mut partial_sum = 0;

    //             for (i, elem) in root.elements().iter().enumerate() {
    //                 let elem_size = elem.size();
    //                 partial_sum += elem_size;
    //                 if key < partial_sum {
    //                     return Some((
    //                         partial_sum - elem_size,
    //                         RefMutBTreeElement(RefMutBTreeElementInner::LeafRoot {
    //                             root,
    //                             elem_idx: i,
    //                             prev_size: elem_size,
    //                         }),
    //                     ));
    //                 }
    //             }
    //             None
    //         }
    //     }
    // }

    pub fn insert(&mut self, pos: usize, key: K, value: V) -> Result<(), (K, V)> {
        let key_size = key.size();
        assert!(0 < key_size);

        self.len += 1;
        match &mut self.root {
            Child::Leaf(root) => {
                let mut partial_sum = 0;

                let overflow_element = 'root_search_and_insert: loop {
                    for (i, elem) in root.elements().iter().enumerate() {
                        let elem_size = elem.size();
                        match pos.cmp(&partial_sum) {
                            Ordering::Less => {
                                self.len -= 1;
                                return Err((key, value));
                            }
                            Ordering::Equal => {
                                match root.insert(i, key, value) {
                                    Some(overflow) => break 'root_search_and_insert overflow,
                                    None => return Ok(()),
                                };
                            }
                            Ordering::Greater => {}
                        }
                        partial_sum += elem_size;
                    }

                    if partial_sum != pos {
                        self.len -= 1;
                        return Err((key, value));
                    }
                    match root.push(key, value) {
                        Some(overflow) => break 'root_search_and_insert overflow,
                        None => return Ok(()),
                    };
                };

                self.depth += 1;

                let mut right = Box::new(NodeElements::new());
                let sep_element = root.split(overflow_element, right.as_mut());

                let left = mem::replace(root, right);
                let new_root = Node::new(Child::Leaf(left));
                let right = mem::replace(&mut self.root, Child::Node(new_root))
                    .try_into_leaf()
                    .unwrap();

                self.root
                    .try_as_node_mut()
                    .unwrap()
                    .push(sep_element, Child::Leaf(right))
                    .assert_none();

                Ok(())
            }
            Child::Node(root) => {
                let mut ref_stack = OnStackRefMutStack::<Node<K, V>, 20>::new();
                ref_stack.push_root(root.as_mut());

                'overflow_loop: loop {
                    let mut children_indices_stack = StackVec::<usize, 20>::new(); // ;
                    let mut partial_sum = 0;

                    let mut overflow_element = 'search_and_insert: loop {
                        let node = ref_stack.peek_mut().unwrap();

                        let mut child_idx = node.num_elements();
                        for (i, elem) in node.elements().iter().enumerate() {
                            let child_size = node.children().get(i).unwrap().size();
                            partial_sum += child_size;
                            if pos <= partial_sum {
                                partial_sum -= child_size;
                                child_idx = i;
                                break;
                            }

                            let elem_size = elem.size();
                            partial_sum += elem_size;
                            if pos < partial_sum {
                                // println!("ERROR(taken_node_element)");
                                self.len -= 1;
                                return Err((key, value));
                            }
                        }

                        children_indices_stack.push(child_idx).assert_none();
                        match ref_stack.try_push(|node| match node.children_mut() {
                            ChildrenSliceMut::Nodes(children_nodes) => {
                                Ok(children_nodes[child_idx].as_mut())
                            }
                            ChildrenSliceMut::Leafs(leafs) => Err(leafs[child_idx].as_mut()),
                        }) {
                            Ok(check) => assert!(check),
                            Err(leaf) => {
                                for (i, elem) in leaf.elements().iter().enumerate() {
                                    let elem_size = elem.size();
                                    match pos.cmp(&partial_sum) {
                                        Ordering::Equal => match leaf.insert(i, key, value) {
                                            Some(overflow) => break 'search_and_insert overflow,
                                            None => break 'overflow_loop,
                                        },
                                        Ordering::Less => {
                                            // println!("ERROR(taken_leaf_element)");
                                            self.len -= 1;
                                            return Err((key, value));
                                        }
                                        Ordering::Greater => {}
                                    }
                                    partial_sum += elem_size;
                                }
                                if partial_sum != pos {
                                    // println!("ERROR(leaf_out_of_bounds)");
                                    self.len -= 1;
                                    return Err((key, value));
                                }
                                match leaf.push(key, value) {
                                    Some(overflow) => break 'search_and_insert overflow,
                                    None => break 'overflow_loop,
                                }
                                // return None;
                            }
                        }
                    };
                    let mut overflow_child;

                    // Leaf Overflow
                    {
                        let node = ref_stack.peek_mut().unwrap();

                        let child_idx = children_indices_stack.pop().unwrap();
                        let child =
                            node.children_mut().try_into_leafs().unwrap()[child_idx].as_mut();

                        let mut right = Box::new(NodeElements::new());
                        let sep_element = child.split(overflow_element, right.as_mut());

                        unsafe {
                            node.set_size(
                                node.size() + key_size - sep_element.size() - right.size(),
                            );
                        }

                        let (rightmost_element, rightmost_child) =
                            match node.insert(child_idx, sep_element, Child::Leaf(right)) {
                                Some(x) => x,
                                None => {
                                    ref_stack.pop();
                                    break 'overflow_loop;
                                }
                            };

                        overflow_element = rightmost_element;
                        overflow_child = rightmost_child;
                    }

                    loop {
                        match ref_stack.pop() {
                            Some(_root) => {
                                drop(ref_stack);

                                self.depth += 1;

                                let (sep_element, right) =
                                    root.split(overflow_element, overflow_child);

                                let left = mem::replace(root, right);

                                let new_root = Node::new(Child::Node(left));
                                let right = mem::replace(root, new_root);

                                root.push(sep_element, Child::Node(right)).assert_none();

                                return Ok(());
                            }
                            None => {
                                let node = ref_stack.peek_mut().unwrap();

                                let child_idx = children_indices_stack.pop().unwrap();
                                let child = node.children_mut().try_into_nodes().unwrap()
                                    [child_idx]
                                    .as_mut();

                                let (sep_element, right) =
                                    child.split(overflow_element, overflow_child);

                                unsafe {
                                    node.set_size(
                                        node.size() + key_size - sep_element.size() - right.size(),
                                    );
                                }

                                let (rightmost_element, rightmost_child) =
                                    match node.insert(child_idx, sep_element, Child::Node(right)) {
                                        Some(x) => x,
                                        None => {
                                            ref_stack.pop();
                                            break 'overflow_loop;
                                        }
                                    };

                                overflow_element = rightmost_element;
                                overflow_child = rightmost_child;
                            }
                        }
                    }
                    // break;
                }

                while let Some(node) = ref_stack.peek_mut() {
                    unsafe {
                        node.set_size(node.size() + key_size);
                    }
                    ref_stack.pop();
                }
                Ok(())
            }
        }
    }

    pub fn remove(&mut self, key: usize) -> Option<(K, V)> {
        fn resolve_underflow<K: OrdSize, V>(node: &mut Node<K, V>, child_idx: usize) {
            debug_assert!(
                node.children().get(child_idx).unwrap().num_elements() < MIN_NUM_ELEMENTS
            );

            let (elements, children) = node.get_all_mut();
            match children {
                ChildrenSliceMut::Nodes(children) => {
                    if let Some(donor_child) = children
                        .get_mut(child_idx + 1)
                        .filter(|child| MIN_NUM_ELEMENTS < child.num_elements())
                    {
                        donor_child.children_mut().swap(0, 1);
                        let (mut removed_element, removed_child) = donor_child.remove(0);

                        mem::swap(&mut elements[child_idx], &mut removed_element);

                        children[child_idx]
                            .push(removed_element, removed_child)
                            .assert_none();
                    } else if let Some(donor_child) = child_idx
                        .checked_sub(1)
                        .map(|i| &mut children[i])
                        .filter(|child| MIN_NUM_ELEMENTS < child.num_elements())
                    {
                        let (mut removed_element, removed_child) = donor_child.pop().unwrap();

                        mem::swap(&mut elements[child_idx - 1], &mut removed_element);

                        children[child_idx]
                            .insert(0, removed_element, removed_child)
                            .assert_none();
                        children[child_idx].children_mut().swap(0, 1);
                    } else {
                        let left = child_idx.saturating_sub(1);

                        let (sep_element, right_child) = node.remove(left);
                        unsafe {
                            node.set_size(node.size() + sep_element.size() + right_child.size());
                        }

                        let left_child = &mut node.children_mut().try_into_nodes().unwrap()[left];

                        let right_child = right_child.try_into_node().unwrap();

                        left_child.merge(sep_element, right_child);
                    }
                }
                ChildrenSliceMut::Leafs(children) => {
                    if let Some(donor_child) = children
                        .get_mut(child_idx + 1)
                        .filter(|child| MIN_NUM_ELEMENTS < child.len())
                    {
                        let mut removed_element = donor_child.remove(0);

                        mem::swap(&mut elements[child_idx], &mut removed_element);

                        children[child_idx].push(removed_element).assert_none();
                    } else if let Some(donor_child) = child_idx
                        .checked_sub(1)
                        .map(|i| &mut children[i])
                        .filter(|child| MIN_NUM_ELEMENTS < child.len())
                    {
                        let mut removed_element = donor_child.pop().unwrap();

                        mem::swap(&mut elements[child_idx - 1], &mut removed_element);

                        children[child_idx].insert(0, removed_element).assert_none();
                    } else {
                        let left = child_idx.saturating_sub(1);

                        let (sep_element, right_child) = node.remove(left);
                        unsafe {
                            node.set_size(node.size() + sep_element.size() + right_child.size());
                        }

                        let left_child = &mut node.children_mut().try_into_leafs().unwrap()[left];

                        let mut right_child = right_child.try_into_leaf().unwrap();

                        left_child.merge(sep_element, right_child.as_mut());
                    }
                }
            }
            debug_assert!(node
                .children()
                .iter()
                .all(|child| MIN_NUM_ELEMENTS <= child.num_elements()));
        }

        match &mut self.root {
            Child::Leaf(root) => {
                let mut partial_sum = 0;
                for (i, elem) in root.elements().iter().enumerate() {
                    let elem_size = elem.size();
                    if key == partial_sum {
                        self.len -= 1;
                        return Some(root.remove(i));
                    }
                    partial_sum += elem_size;
                    if key < partial_sum {
                        return None;
                    }
                }

                None
            }
            Child::Node(root) => {
                let mut partial_sum = 0;

                let mut ref_stack = OnStackRefMutStack::<_, 20>::new();
                let mut children_indices_stack = StackVec::<_, 20>::new(); // ;
                ref_stack.push_root(root.as_mut());

                let removed_element = 'search_and_remove: loop {
                    let node = ref_stack.peek_mut().unwrap();

                    let mut child_idx = node.num_elements();
                    for (i, elem) in node.elements().iter().enumerate() {
                        let child_size = node.children().get(i).unwrap().size();
                        partial_sum += child_size;

                        match key.cmp(&partial_sum) {
                            Ordering::Less => {
                                partial_sum -= child_size;
                                child_idx = i;
                                break;
                            }
                            Ordering::Equal => {
                                children_indices_stack.push(i).assert_none();

                                let replacement_element =
                                    match ref_stack.try_push(|node| match node.children_mut() {
                                        ChildrenSliceMut::Nodes(nodes) => Ok(nodes[i].as_mut()),
                                        ChildrenSliceMut::Leafs(leafs) => Err(leafs[i].as_mut()),
                                    }) {
                                        Ok(check) => {
                                            assert!(check);
                                            loop {
                                                match ref_stack.try_push(|node| {
                                                    match node.children_mut() {
                                                        ChildrenSliceMut::Nodes(nodes) => {
                                                            Ok(nodes.last_mut().unwrap().as_mut())
                                                        }
                                                        ChildrenSliceMut::Leafs(leafs) => {
                                                            Err(leafs.last_mut().unwrap().as_mut())
                                                        }
                                                    }
                                                }) {
                                                    Ok(success) => assert!(success),
                                                    Err(leaf) => {
                                                        break leaf.pop().unwrap();
                                                    }
                                                }
                                            }
                                        }
                                        Err(leaf) => leaf.pop().unwrap(),
                                    };
                                let replacement_element_size = replacement_element.size();

                                while children_indices_stack.len() < ref_stack.len() {
                                    let node = ref_stack.peek_mut().unwrap();

                                    if node
                                        .children()
                                        .get(node.num_elements())
                                        .unwrap()
                                        .num_elements()
                                        < MIN_NUM_ELEMENTS
                                    {
                                        unsafe {
                                            node.set_size(node.size() - replacement_element_size);
                                        }
                                        resolve_underflow(node, node.num_elements());
                                        ref_stack.pop().assert_none();
                                    } else {
                                        while children_indices_stack.len() < ref_stack.len() {
                                            let node = ref_stack.peek_mut().unwrap();

                                            unsafe {
                                                node.set_size(
                                                    node.size() - replacement_element_size,
                                                );
                                            }
                                            ref_stack.pop().assert_none();
                                        }

                                        let node = ref_stack.peek_mut().unwrap();
                                        let removed_element = mem::replace(
                                            &mut node.elements_mut()[i],
                                            replacement_element,
                                        );
                                        let removed_element_size = removed_element.size();
                                        while let Some(node) = ref_stack.peek_mut() {
                                            unsafe {
                                                node.set_size(node.size() - removed_element_size);
                                            }
                                            ref_stack.pop();
                                        }

                                        self.len -= 1;
                                        return Some(removed_element);
                                    }
                                }

                                let node = ref_stack.peek_mut().unwrap();
                                let removed_element =
                                    mem::replace(&mut node.elements_mut()[i], replacement_element);

                                break 'search_and_remove (removed_element);
                            }
                            Ordering::Greater => {}
                        }

                        let elem_size = elem.size();
                        partial_sum += elem_size;
                        if key < partial_sum {
                            return None;
                        }
                    }

                    children_indices_stack.push(child_idx).assert_none();
                    match ref_stack.try_push(|node| match node.children_mut() {
                        ChildrenSliceMut::Nodes(nodes) => Ok(nodes[child_idx].as_mut()),
                        ChildrenSliceMut::Leafs(leafs) => Err(leafs[child_idx].as_mut()),
                    }) {
                        Ok(success) => assert!(success),
                        Err(leaf) => {
                            for (i, elem) in leaf.elements().iter().enumerate() {
                                let elem_size = elem.size();
                                if partial_sum == key {
                                    if leaf.len() == MIN_NUM_ELEMENTS {
                                        break 'search_and_remove leaf.remove(i);
                                    } else {
                                        let removed_element = leaf.remove(i);
                                        let removed_element_size = removed_element.size();

                                        while let Some(node) = ref_stack.peek_mut() {
                                            unsafe {
                                                node.set_size(node.size() - removed_element_size);
                                            }
                                            ref_stack.pop();
                                        }
                                        self.len -= 1;
                                        return Some(removed_element);
                                    }
                                }
                                partial_sum += elem_size;
                                if key < partial_sum {
                                    return None;
                                }
                            }
                            return None;
                        }
                    }
                };
                let removed_element_size = removed_element.size();

                loop {
                    let node = ref_stack.peek_mut().unwrap();
                    let child_idx = children_indices_stack.pop().unwrap();

                    if node.children().get(child_idx).unwrap().num_elements() < MIN_NUM_ELEMENTS {
                        unsafe {
                            node.set_size(node.size() - removed_element_size);
                        }
                        resolve_underflow(node, child_idx);
                    } else {
                        while let Some(node) = ref_stack.peek_mut() {
                            unsafe {
                                node.set_size(node.size() - removed_element_size);
                            }
                            ref_stack.pop();
                        }
                        self.len -= 1;
                        return Some(removed_element);
                    }

                    if let Some(_root) = ref_stack.pop() {
                        drop(ref_stack);

                        if root.num_elements() == 0 {
                            self.depth -= 1;
                            assert!(self.root.replace_with_child());
                        }
                        self.len -= 1;
                        return Some(removed_element);
                    }
                }
            }
        }
    }

    // pub fn check_sizes(&self) {
    //     fn check_sizes_rec<T: OrdSize>(child: ChildRef<T>) -> usize {
    //         match child {
    //             ChildRef::Node(node) => {
    //                 let node_elements_size: usize =
    //                     node.elements().iter().map(|elem| elem.size()).sum();
    //                 let node_children_size: usize = node
    //                     .children()
    //                     .iter()
    //                     .map(|child| check_sizes_rec(child))
    //                     .sum();
    //                 assert_eq!(node.size(), node_elements_size + node_children_size);

    //                 node.size()
    //             }
    //             ChildRef::Leaf(leaf) => {
    //                 let leaf_elements_size = leaf.elements().iter().map(|elem| elem.size()).sum();
    //                 assert_eq!(leaf.size(), leaf_elements_size);
    //                 leaf.size()
    //             }
    //         }
    //     }
    //     check_sizes_rec(self.root.as_ref());
    // }

    pub fn iter(&self) -> BTreeIter<K, V> {
        let mut left = Vec::with_capacity(self.depth());
        left.push((self.root.as_ref(), 0));
        while let Some(&(ChildRef::Node(node), _)) = left.last() {
            left.push((node.children().get(0).unwrap(), 0));
        }

        let mut right = Vec::with_capacity(self.depth());
        right.push((self.root.as_ref(), self.root.num_elements()));
        while let Some(&(ChildRef::Node(node), child_idx)) = right.last() {
            let child = node.children().get(child_idx).unwrap();
            right.push((child, child.num_elements()));
        }

        BTreeIter {
            left,
            right,
            len: self.len(),
        }
    }

    // pub fn iter_mut(&mut self) -> BTreeIterMut<K, V> {
    //     unsafe {
    //         let mut left = Vec::with_capacity(self.depth());
    //         left.push((
    //             match self.root.as_mut() {
    //                 ChildRefMut::Node(root) => ChildPtrMut::Node(root),
    //                 ChildRefMut::Leaf(root) => ChildPtrMut::Leaf(root),
    //             },
    //             0,
    //         ));

    //         while let Some(&(ChildPtrMut::Node(node), _)) = left.last() {
    //             left.push((
    //                 match (*node).children_mut() {
    //                     ChildrenSliceMut::Nodes(children) => ChildPtrMut::Node(&mut *children[0]),
    //                     ChildrenSliceMut::Leafs(children) => ChildPtrMut::Leaf(&mut *children[0]),
    //                 },
    //                 0,
    //             ));
    //         }

    //         let mut right = Vec::with_capacity(self.depth());
    //         right.push((
    //             match self.root.as_mut() {
    //                 ChildRefMut::Node(root) => ChildPtrMut::Node(root),
    //                 ChildRefMut::Leaf(root) => ChildPtrMut::Leaf(root),
    //             },
    //             self.root.num_elements(),
    //         ));
    //         while let Some(&(ChildPtrMut::Node(node), child_idx)) = right.last() {
    //             right.push(match (*node).children_mut() {
    //                 ChildrenSliceMut::Nodes(children) => {
    //                     let child = &mut *children[child_idx];
    //                     (ChildPtrMut::Node(child), child.num_elements())
    //                 }
    //                 ChildrenSliceMut::Leafs(children) => {
    //                     let child = &mut *children[child_idx];
    //                     (ChildPtrMut::Leaf(child), child.len())
    //                 }
    //             });
    //         }

    //         BTreeIterMut {
    //             left,
    //             right,
    //             len: self.len(),
    //             phantom: PhantomData,
    //         }
    //     }
    // }
}

impl<K: OrdSize, V> Default for OrdBTree<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct BTreeIter<'a, K: OrdSize, V> {
    left: Vec<(ChildRef<'a, K, V>, usize)>,
    right: Vec<(ChildRef<'a, K, V>, usize)>,
    len: usize,
}

impl<'a, K: OrdSize, V> Iterator for BTreeIter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        if 0 < self.len {
            self.len -= 1;
            let (child, elem_idx) = self.left.last_mut().unwrap();
            let item = &child.elements()[*elem_idx];

            *elem_idx += 1;
            match *child {
                ChildRef::Node(_) => {
                    while let Some(&(ChildRef::Node(node), child_idx)) = self.left.last() {
                        self.left.push((node.children().get(child_idx).unwrap(), 0));
                    }
                }
                ChildRef::Leaf(_) => {
                    while let Some(&(child, elem_idx)) = self.left.last() {
                        if child.num_elements() <= elem_idx {
                            self.left.pop();
                        }
                    }
                }
            }

            Some(item)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }

    fn count(self) -> usize {
        self.len
    }
}

impl<'a, K: OrdSize, V> DoubleEndedIterator for BTreeIter<'a, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if 0 < self.len {
            self.len -= 1;
            let (child, elem_idx) = self.right.last_mut().unwrap();
            *elem_idx -= 1;
            let item = &child.elements()[*elem_idx];

            match *child {
                ChildRef::Node(_) => {
                    while let Some(&(ChildRef::Node(node), child_idx)) = self.right.last() {
                        let child = node.children().get(child_idx).unwrap();
                        self.right.push((child, child.num_elements()));
                    }
                }
                ChildRef::Leaf(_) => {
                    while let Some(&(_, elem_idx)) = self.right.last() {
                        if elem_idx == 0 {
                            self.right.pop();
                        }
                    }
                }
            }

            Some(item)
        } else {
            None
        }
    }
}

impl<'a, K: OrdSize, V> ExactSizeIterator for BTreeIter<'a, K, V> {
    fn len(&self) -> usize {
        self.len
    }
}

// #[derive(Debug)]
// pub struct BTreeIterMut<'a, K: Ord, V> {
//     left: Vec<(ChildPtrMut<K, V>, usize)>,
//     right: Vec<(ChildPtrMut<K, V>, usize)>,
//     len: usize,
//     phantom: PhantomData<ChildRefMut<'a, K, V>>,
// }
//
// impl<'a, K: Ord, V> Iterator for BTreeIterMut<'a, K, V> {
//     type Item = (&'a K, &'a mut V);
//
//     fn next(&mut self) -> Option<Self::Item> {
//         if 0 < self.len {
//             self.len -= 1;
//             let (child, elem_idx) = self.left.last_mut().unwrap();
//
//             *elem_idx += 1;
//             match *child {
//                 ChildPtrMut::Node(node) => unsafe {
//                     let node = &mut *node;
//                     let (keys, values, _) = node.get_all_mut();
//                     let item = (&keys[*elem_idx], &mut values[*elem_idx]);
//
//                     while let Some(&(ChildPtrMut::Node(node), child_idx)) = self.left.last() {
//                         self.left.push((
//                             match (*node).children_mut() {
//                                 ChildrenSliceMut::Nodes(children) => {
//                                     ChildPtrMut::Node(&mut *children[child_idx])
//                                 }
//                                 ChildrenSliceMut::Leafs(children) => {
//                                     ChildPtrMut::Leaf(&mut *children[child_idx])
//                                 }
//                             },
//                             0,
//                         ));
//                     }
//                     Some(item)
//                 },
//                 ChildPtrMut::Leaf(leaf) => unsafe {
//                     let leaf = &mut *leaf;
//                     let (keys, values) = leaf.get_all_mut();
//                     let item = (&keys[*elem_idx], &mut values[*elem_idx]);
//
//                     while let Some(&(ref child, elem_idx)) = self.left.last() {
//                         if match child {
//                             ChildPtrMut::Leaf(leaf) => (**leaf).len(),
//                             ChildPtrMut::Node(node) => (**node).num_elements(),
//                         } <= elem_idx
//                         {
//                             self.left.pop();
//                         }
//                     }
//                     Some(item)
//                 },
//             }
//         } else {
//             None
//         }
//     }
//
//     fn size_hint(&self) -> (usize, Option<usize>) {
//         (self.len, Some(self.len))
//     }
//
//     fn count(self) -> usize {
//         self.len
//     }
// }
//
// impl<'a, K: Ord, V> DoubleEndedIterator for BTreeIterMut<'a, K, V> {
//     fn next_back(&mut self) -> Option<Self::Item> {
//         if 0 < self.len {
//             self.len -= 1;
//             let (child, elem_idx) = self.right.last_mut().unwrap();
//             *elem_idx -= 1;
//
//             match *child {
//                 ChildPtrMut::Node(node) => unsafe {
//                     let node = &mut *node;
//                     let (keys, values, _) = node.get_all_mut();
//                     let item = (&keys[*elem_idx], &mut values[*elem_idx]);
//
//                     while let Some(&(ChildPtrMut::Node(node), child_idx)) = self.right.last() {
//                         self.right.push(match (*node).children_mut() {
//                             ChildrenSliceMut::Nodes(children) => {
//                                 let child = &mut *children[child_idx];
//                                 (ChildPtrMut::Node(child), child.num_elements())
//                             }
//                             ChildrenSliceMut::Leafs(children) => {
//                                 let child = &mut *children[child_idx];
//                                 (ChildPtrMut::Leaf(child), child.len())
//                             }
//                         });
//                     }
//                     Some(item)
//                 },
//                 ChildPtrMut::Leaf(leaf) => unsafe {
//                     let leaf = &mut *leaf;
//                     let (keys, values) = leaf.get_all_mut();
//                     let item = (&keys[*elem_idx], &mut values[*elem_idx]);
//
//                     while let Some(&(_, elem_idx)) = self.right.last() {
//                         if elem_idx == 0 {
//                             self.right.pop();
//                         }
//                     }
//                     Some(item)
//                 },
//             }
//         } else {
//             None
//         }
//     }
// }
//
// impl<'a, K: Ord, V> ExactSizeIterator for BTreeIterMut<'a, K, V> {
//     fn len(&self) -> usize {
//         self.len
//     }
// }
