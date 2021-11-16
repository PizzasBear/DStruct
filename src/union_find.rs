pub struct UnionFind {
    parents: Vec<usize>,
    sizes: Vec<usize>,
}

impl UnionFind {
    #[inline]
    pub fn new(size: usize) -> Self {
        Self {
            parents: (0..size).collect(),
            sizes: vec![1; size],
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.parents.len()
    }

    pub fn increase_size(&mut self, additional: usize) {
        let len = self.len();
        let new_len = len + additional;

        self.parents.extend(len..new_len);
        self.sizes.resize(new_len, 0);
    }

    pub fn find(&mut self, mut i: usize) -> usize {
        let mut root = i;
        while self.parents[root] != root {
            root = self.parents[root];
        }

        while i != root {
            let parent = self.parents[i];
            self.parents[i] = root;
            i = parent;
        }

        root
    }

    pub fn union(&mut self, mut i: usize, mut j: usize) -> bool {
        i = self.find(i);
        j = self.find(j);

        if i == j {
            false
        } else {
            if self.sizes[i] < self.sizes[j] {
                self.parents[i] = j;
                self.sizes[j] += self.sizes[i];
            } else {
                self.parents[j] = i;
                self.sizes[i] += self.sizes[j];
            }

            true
        }
    }

    #[inline]
    pub fn is_root(&self, root: usize) -> bool {
        root == self.parents[root]
    }

    pub fn size(&self, root: usize) -> Option<usize> {
        if self.is_root(root) {
            Some(self.sizes[root])
        } else {
            None
        }
    }

    /// `root` has to be a root for the output to make sense. Cuts off unnecessary branching.
    #[inline]
    pub fn size_unchecked(&self, root: usize) -> usize {
        self.sizes[root]
    }

    #[inline]
    pub fn size_find(&mut self, i: usize) -> usize {
        let root = self.find(i);
        self.sizes[root]
    }
}
