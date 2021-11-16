use crate::groups::{AbelianGroup, CommutativeMonoid};

#[inline]
fn lsb<I: num::PrimInt>(n: I) -> I {
    n & (!n + I::one())
}

#[derive(Clone, Debug)]
pub struct FenwickTree<G: CommutativeMonoid> {
    group: G,
    data: Vec<G::Elem>,
}

impl<G: CommutativeMonoid> FenwickTree<G> {
    #[inline]
    pub fn new(group: G) -> Self {
        Self {
            group,
            data: Vec::new(),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    #[inline]
    fn range_start(mut i: usize) -> usize {
        i += 1;
        i - lsb(i)
    }

    /// O(log n)
    pub fn update_add(&mut self, mut i: usize, dx: G::Elem) {
        while i < self.len() {
            // TODO: Replace `self.data[i].clone()` with something more appropriate (maybe unsafe).
            self.data[i] = self.group.add(self.data[i].clone(), dx.clone());
            i |= lsb(!i);
        }
    }

    /// O(log n)
    pub fn prefix_sum(&self, mut i: usize) -> G::Elem {
        let mut ps = self.group.id();
        while i != 0 {
            ps = self.group.add(self.data[i - 1].clone(), ps);
            i -= lsb(i); // i is going to the left of `ps`
        }
        ps
    }

    /// O(1)
    pub fn push_id(&mut self) {
        self.data.push(self.group.id());
    }

    /// O(n)
    pub fn resize_id(&mut self, size: usize) {
        self.data.resize(size, self.group.id());
    }

    /// Simular to `get`, therefore has similar performance.
    ///
    /// Avg: O(1), Worst case: O(log n)
    pub fn push(&mut self, mut x: G::Elem) {
        let bottom = Self::range_start(self.len());

        let mut i = self.len();
        while bottom < i {
            x = self.group.add(self.data[i - 1].clone(), x);
            i -= lsb(i); // i is going to the left of `x`
        }

        self.data.push(x);
    }

    /// O(1)
    #[inline]
    pub fn pop(&mut self) {
        self.data.pop();
    }

    /// Constructs a new, empty `FenwickTree<G>` with the specified capacity.
    #[inline]
    pub fn with_capacity(group: G, capacity: usize) -> Self {
        Self {
            group,
            data: Vec::with_capacity(capacity),
        }
    }

    /// Reserves capacity for at least `additional` more elements to be inserted
    /// in the given `FenwickTree<G>`. The collection may reserve more space to avoid
    /// frequent reallocations. After calling `reserve`, capacity will be
    /// greater than or equal to `self.len() + additional`. Does nothing if
    /// capacity is already sufficient.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.data.reserve(additional);
    }

    /// O(n)
    pub fn resize(&mut self, size: usize, x: G::Elem) {
        while self.len() < size {
            self.push(x.clone());
        }
        self.data.resize(size, self.group.id());
    }

    /// O(n)
    pub fn resize_with<F: FnMut() -> G::Elem>(&mut self, size: usize, f: &mut F) {
        while self.len() < size {
            self.push(f());
        }
        self.data.resize(size, self.group.id());
    }
}

impl<G: AbelianGroup> FenwickTree<G> {
    /// `update_add` should be perferred over this, because it's faster (it doesn't call `get`).
    ///
    /// O(log n)
    pub fn update_set(&mut self, i: usize, x: G::Elem) {
        self.update_add(i, self.group.sub(x, self.get(i)));
    }

    /// Avg: O(1), Worst case: O(log i)
    pub fn get(&self, mut i: usize) -> G::Elem {
        let mut x = self.data[i].clone();
        let bottom = Self::range_start(i);
        while bottom < i {
            x = self.group.sub(x, self.data[i - 1].clone());
            i -= lsb(i);
        }
        x
    }
}

impl<G: CommutativeMonoid> Extend<G::Elem> for FenwickTree<G> {
    fn extend<Iter: IntoIterator<Item = G::Elem>>(&mut self, iter: Iter) {
        let iter = iter.into_iter();
        match iter.size_hint() {
            (_, Some(len)) => self.reserve(len),
            (len, None) => self.reserve(len),
        }

        for x in iter {
            self.push(x);
        }
    }
}
