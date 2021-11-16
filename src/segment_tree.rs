use crate::groups::CommutativeMonoid;

#[derive(Clone, Debug)]
pub struct SegmentTree<G: CommutativeMonoid> {
    group: G,
    data: Vec<G::Elem>,
}

impl<G: CommutativeMonoid> SegmentTree<G> {
    #[inline]
    pub fn new(group: G, length: usize) -> Self {
        let mut data = Vec::new();
        data.resize_with(2 * length - 1, || group.id());
        Self { group, data }
    }

    pub fn len(&self) -> usize {
        (self.data.len() + 1) / 2
    }

    fn start(&self) -> usize {
        self.data.len() / 2
    }

    /// O(n)
    pub fn build<Iter: IntoIterator<Item = G::Elem>>(&mut self, iter: Iter) {
        let start = self.start();
        for (i, x) in iter.into_iter().enumerate() {
            self.data[start + i] = x;
        }
        for i in (0..start).rev() {
            let l = 2 * i + 1;
            let r = l + 1;
            self.data[i] = self.group.add(self.data[l].clone(), self.data[r].clone());
        }
    }

    /// O(log n)
    pub fn sum(&self, mut l: usize, mut r: usize) -> G::Elem {
        let start = self.start();
        l += start;
        r += start;

        let mut s = self.group.id();
        while l < r {
            if l & 1 == 0 {
                s = self.group.add(s, self.data[l].clone());
            }
            if r & 1 == 0 {
                r -= 1;
                s = self.group.add(s, self.data[r].clone());
            }
            l /= 2;
            r /= 2;
        }

        s
    }

    /// O(log n)
    pub fn update(&mut self, mut i: usize, x: G::Elem) {
        i += self.start();
        self.data[i] = x;

        while i != 0 {
            i = (i - 1) / 2;

            let l = 2 * i + 1;
            let r = l + 1;
            self.data[i] = self.group.add(self.data[l].clone(), self.data[r].clone());
        }
    }

    /// O(1)
    pub fn get(&self, i: usize) -> &G::Elem {
        &self.data[self.start() + i]
    }
}
