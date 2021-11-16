#[derive(Clone, Default, Debug)]
pub struct MaxHeap<T: Ord> {
    data: Vec<T>,
}

pub type MinHeap<T> = MaxHeap<std::cmp::Reverse<T>>;

impl<T: Ord> MaxHeap<T> {
    /// O(1)
    #[inline]
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// O(1)
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    /// O(1)
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// O(1)
    #[inline]
    pub fn peek(&self) -> Option<&T> {
        self.data.first()
    }

    /// O(1)
    pub fn reserve(&mut self, additional: usize) {
        self.data.reserve(additional);
    }

    /// O(log n)
    pub fn pop(&mut self) -> Option<T> {
        use std::mem;

        let mut res = self.data.pop()?;
        res = mem::replace(self.data.first_mut()?, res);
        self.sift_down(0);

        Some(res)
    }

    /// O(log n)
    pub fn push(&mut self, value: T) {
        self.data.push(value);
        self.sift_up(self.len() - 1);
    }

    /// O(log n)
    fn sift_up(&mut self, mut node: usize) {
        while node != 0 {
            let parent = (node - 1) / 2;

            if self.data[parent] < self.data[parent] {
                self.data.swap(parent, node);
                node = parent;
            } else {
                break;
            }
        }
    }

    /// O(log n)
    fn sift_down(&mut self, mut node: usize) {
        loop {
            let left = 2 * node + 1;
            let right = left + 1;

            if right < self.len() {
                let max = if self.data[left] < self.data[right] {
                    right
                } else {
                    left
                };

                if self.data[node] < self.data[max] {
                    self.data.swap(node, max);
                    node = max;
                } else {
                    break;
                }
            } else {
                if right == self.len() && self.data[left] < self.data[node] {
                    self.data.swap(node, left);
                }
                break;
            }
        }
    }
}

impl<T: Ord> std::iter::FromIterator<T> for MaxHeap<T> {
    // TODO: O(n log n) -> O(n)
    fn from_iter<Iter: IntoIterator<Item = T>>(iter: Iter) -> Self {
        let mut heap = Self {
            data: iter.into_iter().collect(),
        };

        for i in (0..heap.len() / 2).rev() {
            heap.sift_down(i);
        }

        heap
    }
}
