#![allow(dead_code)]

use ordered_float::*;
use std::collections::{BinaryHeap, VecDeque};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Code {
    pub bits: u32,
    pub len: u8,
}

impl Code {
    pub const EMPTY: Self = Self { bits: 0, len: 0 };

    #[inline]
    pub const fn new(bits: u32, len: u8) -> Self {
        Self { bits, len }
    }
}

pub struct Huffman {
    pub encoding: Vec<Code>,
}

// IEEE 754 <-> uniform 32 bit float [0..1)
// ----------------------------------------
//
// pub fn f01_to_u32(f: f32) -> u32 {
//     assert!(0. <= f && f <= 1.);
//     if f == 1. {
//         !0
//     } else {
//         let bits = f.to_bits();
//
//         let exp = 127 - (255 & bits >> 23);
//         let frac = (1 << 23) + ((1 << 23) - 1 & bits);
//
//         if exp < 10 {
//             frac << 9 - exp
//         } else if exp < 24 + 9 {
//             frac >> exp - 9
//         } else {
//             0
//         }
//     }
// }
//
// pub fn u32_to_f01(u: u32) -> f32 {
//     if 0xffffff7f < u {
//         1.0
//     } else if u == 0 {
//         0.0
//     } else {
//         let mut bits = 0;
//         let lz = u.leading_zeros();
//         bits |= 126 - lz << 23;
//
//         bits |= (1 << 23) - 1 & if lz < 9 { u >> 8 - lz } else { u << lz - 8 };
//
//         f32::from_bits(bits)
//     }
// }

impl Huffman {
    pub fn new(probs: &[f64]) -> Self {
        use std::cmp::Ordering;

        #[derive(Default, Clone, Copy, PartialEq, Eq)]
        struct Node {
            pub code: Code,
            pub left_child: usize,
            pub right_child: usize,
        }

        impl Node {
            #[inline]
            pub const fn new(left_child: usize, right_child: usize) -> Self {
                Self {
                    code: Code::EMPTY,
                    left_child,
                    right_child,
                }
            }

            #[inline]
            pub const fn leaf(id: usize) -> Self {
                Self {
                    code: Code::EMPTY,
                    left_child: id,
                    right_child: id,
                }
            }

            #[inline]
            pub const fn is_leaf(&self) -> bool {
                self.left_child == self.right_child
            }
        }

        #[derive(Default, Clone, Copy, PartialEq, Eq)]
        struct HeapCell {
            pub id: usize,
            pub prob: OrderedFloat<f64>,
        }

        impl HeapCell {
            #[inline]
            pub const fn new(id: usize, prob: OrderedFloat<f64>) -> Self {
                Self { prob, id }
            }
        }

        impl PartialOrd for HeapCell {
            #[inline]
            fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
                rhs.prob.partial_cmp(&self.prob)
            }
        }

        impl Ord for HeapCell {
            #[inline]
            fn cmp(&self, rhs: &Self) -> Ordering {
                rhs.prob.cmp(&self.prob)
            }
        }

        let mut heap: BinaryHeap<HeapCell> = probs
            .iter()
            .enumerate()
            .map(|(i, &p)| HeapCell::new(i, p.into()))
            .collect();
        let mut forest: Vec<Node> = (0..probs.len()).map(|i| Node::leaf(i)).collect();

        while 1 < heap.len() {
            let parent_id = forest.len();

            let (p, q) = (heap.pop().unwrap(), heap.pop().unwrap());

            heap.push(HeapCell {
                id: parent_id,
                prob: p.prob + q.prob,
            });
            forest.push(Node::new(p.id, q.id));
        }

        assert!(heap.peek().unwrap().prob.abs_sub(1.0.into()) <= OrderedFloat(f64::EPSILON));

        let mut bfs_queue = VecDeque::with_capacity(forest.len());
        bfs_queue.push_back(forest.len() - 1);
        while !bfs_queue.is_empty() {
            let node = forest[bfs_queue.pop_front().unwrap()];

            let mut new_code = Code::new(2 * node.code.bits, node.code.len + 1);
            let left_child = &mut forest[node.left_child];
            left_child.code = new_code;
            if !left_child.is_leaf() {
                bfs_queue.push_back(node.left_child)
            }

            new_code.bits |= 1;
            let right_child = &mut forest[node.left_child];
            right_child.code = new_code;
            if !right_child.is_leaf() {
                bfs_queue.push_back(node.right_child)
            }
        }

        Self {
            encoding: forest[0..probs.len()]
                .iter()
                .map(|node| node.code)
                .collect(),
        }
    }

    #[allow(unreachable_code)]
    pub fn encode<Iter: IntoIterator<Item = usize>>(_iter: Iter) -> impl Iterator<Item = u8> {
        todo!();
        0..255
    }
}
