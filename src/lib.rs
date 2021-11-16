pub mod groups;

mod btree;
mod complex;
mod fenwick_tree;
mod fft;
mod heap;
mod huffman;
// mod ord_btree;
mod ref_stack;
mod segment_tree;
mod stack_vec;
mod union_find;

// use complex::Complex64;
use fft::{eval_poly, fft2, ifft2, PrintPoly};
// use huffman::Huffman;

pub use btree::BTree;
pub use fenwick_tree::FenwickTree;
pub use heap::{MaxHeap, MinHeap};
// pub use ord_btree::{OrdBTree, OrdSize, OrdSizeOne}; // , RefMutBTreeElement};
pub use ref_stack::{OnStackRefMutStack, RefMutStack};
pub use segment_tree::SegmentTree;
pub use stack_vec::{
    OuterLenStackVec, OuterLenStackVecDrain, StackVec, StackVecDrain, StackVecIntoIter,
};
pub use union_find::UnionFind;

use rand::prelude::*;

fn bench<F: FnOnce()>(name: &str, num_tabs: usize, f: F) {
    use std::time::{Duration, Instant};
    let start = Instant::now();
    f();
    let elapsed = start.elapsed();

    print!("BENCH `{}` :", name);
    for _ in 0..num_tabs {
        print!("\t");
    }

    if elapsed < Duration::from_millis(1) {
        println!(
            "{} {:03} nanos",
            elapsed.as_micros(),
            elapsed.as_nanos() % 1000,
        );
    } else if elapsed < Duration::from_secs(1) {
        println!(
            "{} {:03} micros",
            elapsed.as_millis(),
            elapsed.as_micros() % 1000,
        );
    } else {
        println!(
            "{} {:03} millis",
            elapsed.as_secs(),
            elapsed.subsec_millis(),
        );
    }
}

#[allow(dead_code)]
fn bench_prefix_sum_dstruct() {
    let mut rng = SmallRng::from_entropy();

    const N: usize = 1 << 20; //1000_000;

    let mut a: Vec<_> = (0..N as i32).collect();
    a.shuffle(&mut rng);

    let mut st = SegmentTree::new(groups::NumAdditiveGroups::<i32>::new(), N);
    let mut ft = FenwickTree::new(groups::NumAdditiveGroups::<i32>::new());
    ft.reserve(N);

    bench("SegmentTree::build", 1, || st.build(a.iter().cloned()));
    bench("FenwickTree::extend", 1, || ft.extend(a.iter().cloned()));
    println!();

    bench("SegmentTree::prefix_sum", 1, || {
        for i in 0..N {
            st.sum(0, i);
        }
    });
    bench("FenwickTree::prefix_sum", 1, || {
        for i in 0..N {
            ft.prefix_sum(i);
        }
    });
    println!();

    bench("SegmentTree::update_add", 1, || {
        for i in 0..N {
            st.update(i, i as i32 + st.get(i));
        }
    });
    bench("FenwickTree::update_add", 1, || {
        for i in 0..N {
            ft.update_add(i, i as _);
        }
    });
    println!();

    bench("SegmentTree::update_set", 1, || {
        for (i, &x) in a.iter().enumerate() {
            st.update(i, x);
        }
    });
    bench("FenwickTree::update_set", 1, || {
        for (i, &x) in a.iter().enumerate() {
            ft.update_set(i, x);
        }
    });
    println!();

    assert_eq!(ft.get(3 * N / 4), a[3 * N / 4]);
    assert_eq!(*st.get(3 * N / 4), a[3 * N / 4]);
}

#[allow(dead_code)]
fn bench_fft_dstruct() {
    use num::{one, zero};

    let mut rng = SmallRng::from_entropy();
    let mut p: Vec<num::Complex<f64>> = vec![zero(); 0x20_000];

    bench("baseline(placebo)", 1, || {
        for _ in 0..20 {
            for x in p.iter_mut() {
                *x = num::Complex::new(rng.gen_range(-2.0..2.0), rng.gen_range(-2.0..2.0));
            }
        }
    });
    bench("ifft2", 1, || {
        for _ in 0..20 {
            for x in p.iter_mut() {
                *x = num::Complex::new(rng.gen_range(-2.0..2.0), rng.gen_range(-2.0..2.0));
            }
            ifft2(&mut p);
        }
    });
    bench("fft2", 1, || {
        for _ in 0..20 {
            for x in p.iter_mut() {
                *x = num::Complex::new(rng.gen_range(-2.0..2.0), rng.gen_range(-2.0..2.0));
            }
            fft2(&mut p);
        }
    });
    let p_mid = p.len() / 2;
    p[..p_mid].fill(zero());
    p[p_mid..].fill(one());
    fft2(&mut p);
    println!(
        "{:.3}",
        PrintPoly("p", &p.iter().step_by(p.len() / 32).collect::<Vec<_>>())
    );
    println!(
        "{}",
        eval_poly(&p, num::Complex::new(0.0, std::f64::consts::PI * 0.5).exp())
            / num::Complex::new(p.len() as f64, 0.0),
    );
}

#[allow(dead_code)]
fn validate_btree_dstruct() {
    let mut rng = SmallRng::from_entropy();
    let mut map = BTree::new();

    const K: usize = 64; // 4;
    const N: usize = K * 1024; // 8;

    let mut values = Vec::with_capacity(N);
    for _ in 0..N {
        values.push(rng.gen_range(0..1000_000))
    }

    let mut insert_perm: Vec<_> = (0..N).collect();
    insert_perm.shuffle(&mut rng);

    let mut get_perm: Vec<_> = insert_perm.clone();
    let mut remove_perm: Vec<_> = insert_perm.clone();

    println!("[Validate custom BTree]");
    // println!("[Insertion Test]");
    for k in 0..K {
        // println!("k = {}", k);
        let start = k * N / K;
        let end = start + N / K;

        get_perm[start..end].shuffle(&mut rng);

        for &i in insert_perm[start..end].iter() {
            // println!("map IS {:#?};", map);
            // println!("INSERT ({}, {}) INTO map;", i, values[i]);
            assert!(map.insert(i, values[i]).is_none());
            // println!();
        }
        // println!("map IS {:#?};", map);

        for &i in get_perm[..end].iter() {
            // println!("SELECT value FROM map WHERE k={};", i);
            assert_eq!(map.get(&i), Some(&values[i]));
        }
    }

    // println!("[\"Removal Test\"]");
    for k in (0..K).rev() {
        // println!("k = {}", k);
        let start = k * N / K;
        let end = start + N / K;

        remove_perm[start..end].shuffle(&mut rng);

        for &i in remove_perm[start..end].iter() {
            // println!("map IS {:#?};", map);
            // println!("REMOVE (k, value) FROM map WHERE k={};", i);
            assert_eq!(map.remove(&i), Some((i, values[i])));
            // println!();
        }

        for &i in get_perm[..start].iter() {
            // println!("SELECT value FROM map WHERE k={};", i);
            assert_eq!(map.get(&i), Some(&values[i]));
        }
    }
    println!("BTree VALIDATED");
    println!();
}

// #[allow(dead_code)]
// fn validate_ord_btree_dstruct() {
//     let mut rng = SmallRng::from_entropy();
//     let mut map = OrdBTree::new();
//
//     const K: usize = 64; // 16;
//     const N: usize = K * 1024;
//     // const K: usize = 4;
//     // const N: usize = K * 8;
//
//     let mut true_map = Vec::with_capacity(N);
//
//     let mut insert_perm: Vec<_> = (0..N).collect();
//     insert_perm.shuffle(&mut rng);
//
//     let mut get_perm: Vec<_> = (0..N).collect();
//     let mut remove_perm: Vec<_> = (0..N).collect();
//     remove_perm.shuffle(&mut rng);
//
//     println!("[Validate OrdBTree]");
//     print!("Insertion Test: ");
//     std::io::Write::flush(&mut std::io::stdout().lock()).unwrap();
//     for k in 0..K {
//         // println!("k = {}", k);
//         let start = k * N / K;
//         let end = start + N / K;
//
//         get_perm[start..end].shuffle(&mut rng);
//
//         for &val in insert_perm[start..end].iter() {
//             let i = true_map.binary_search(&val).unwrap_err();
//             true_map.insert(i, val);
//
//             // println!("map IS {:#?};", map);
//             // println!("INSERT AT {} value={} INTO map;", i, val);
//             // map.check_sizes();
//             assert!(map.insert(i, OrdSizeOne, val).is_ok());
//             // println!();
//         }
//         // println!("map IS {:#?};", map);
//
//         for &i in get_perm[..end].iter() {
//             // println!("SELECT value AT {} FROM map;", i);
//             assert_eq!(map.get(i), Some((i, &OrdSizeOne, &true_map[i])));
//         }
//     }
//     println!("COMPLETE");
//
//     print!("Removal Test: ");
//     std::io::Write::flush(&mut std::io::stdout().lock()).unwrap();
//     for k in (0..K).rev() {
//         // println!("k = {}", k);
//         let start = k * N / K;
//         let end = start + N / K;
//
//         for &val in remove_perm[start..end].iter() {
//             let i = true_map.binary_search(&val).unwrap();
//             assert_eq!(true_map.remove(i), val);
//
//             // println!("map IS {:#?};", map);
//             // println!("REMOVE value {} AT {} FROM map;", val, i);
//             // map.check_sizes();
//             assert_eq!(map.remove(i), Some((OrdSizeOne, val)));
//             // println!();
//         }
//
//         for &i in get_perm[..start].iter() {
//             // println!("SELECT value AT {} FROM map;", i);
//             assert_eq!(map.get(i), Some((i, &OrdSizeOne, &true_map[i])));
//         }
//     }
//     println!("COMPLETE");
//     println!();
//     println!("OrdBTree VALIDATED");
//     println!();
// }

#[allow(dead_code)]
fn bench_btree_dstruct() {
    let mut rng = SmallRng::from_entropy();

    const N: usize = 256 * 1024; // 256 KiB

    let values: Vec<_> = (0..N).map(|_| rng.gen_range(0..1000_000)).collect();
    let (insert_perm, get_perm, remove_perm) = {
        let mut insert_perm: Vec<_> = (0..N).collect();
        let mut get_perm: Vec<_> = (0..N).collect();
        let mut remove_perm: Vec<_> = (0..N).collect();

        insert_perm.shuffle(&mut rng);
        get_perm.shuffle(&mut rng);
        remove_perm.shuffle(&mut rng);

        (insert_perm, get_perm, remove_perm)
    };

    drop(
        insert_perm
            .iter()
            .map(|&i| (i, values[i]))
            .collect::<std::collections::BTreeMap<_, _>>(),
    );

    let mut std_map = std::collections::BTreeMap::new();
    bench("std::collections::BTreeMap::insert", 2, || {
        for &i in insert_perm.iter() {
            assert!(std_map.insert(i, values[i]).is_none());
        }
    });
    bench("std::collections::BTreeMap::get", 2, || {
        for &i in get_perm.iter() {
            assert_eq!(std_map.get(&i), Some(&values[i]));
        }
    });
    bench("std::collections::BTreeMap::remove_entry", 1, || {
        for &i in remove_perm.iter() {
            assert_eq!(std_map.remove_entry(&i), Some((i, values[i])));
        }
    });
    println!();

    let mut map = BTree::new();
    bench("BTree::insert", 5, || {
        for (len, &i) in insert_perm.iter().enumerate() {
            assert_eq!(map.len(), len);
            assert!(map.insert(i, values[i]).is_none());
            assert_eq!(map.len(), len + 1);
        }
    });
    bench("BTree::get", 5, || {
        for &i in get_perm.iter() {
            assert_eq!(map.get(&i), Some(&values[i]));
        }
    });
    bench("BTree::remove", 5, || {
        for (len, &i) in remove_perm.iter().enumerate() {
            assert_eq!(map.len(), N - len);
            assert_eq!(map.remove(&i), Some((i, values[i])));
            assert_eq!(map.len(), N - len - 1);
        }
    });
}

// #[allow(dead_code)]
// fn bench_ord_btree_dstruct() {
//     let mut rng = SmallRng::from_entropy();
//
//     const N: usize = 256 * 1024; // 256 KiB
//
//     let (insert_perm, get_perm, remove_perm) = {
//         let insert_perm: Vec<_> = (0..N).map(|n| rng.gen_range(0..=n)).collect();
//         let remove_perm: Vec<_> = (1..=N).rev().map(|n| rng.gen_range(0..n)).collect();
//         let mut get_perm: Vec<_> = (0..N).collect();
//
//         get_perm.shuffle(&mut rng);
//
//         (insert_perm, get_perm, remove_perm)
//     };
//
//     drop(get_perm.iter().collect::<std::collections::BTreeSet<_>>());
//
//     let mut map = OrdBTree::new();
//     bench("OrdBTree::insert", 4, || {
//         for (len, &i) in insert_perm.iter().enumerate() {
//             assert_eq!(map.len(), len);
//             assert!(map.insert(i, OrdSizeOne, len).is_ok()); // lets hope it's correct (:
//             assert_eq!(map.len(), len + 1);
//         }
//     });
//     bench("OrdBTree::get", 5, || {
//         for &i in get_perm.iter() {
//             assert!(map.get(i).is_some()); // lets hope it's correct (:
//         }
//     });
//     bench("OrdBTree::remove", 4, || {
//         for (len, &i) in remove_perm.iter().enumerate() {
//             assert_eq!(map.len(), N - len);
//             assert!(map.remove(i).is_some()); // lets hope it's correct (:
//             assert_eq!(map.len(), N - len - 1);
//         }
//     });
// }

#[allow(dead_code)]
fn valgrind_btree_dstruct() {
    const N: usize = 128 * 1024;
    let mut rng = SmallRng::from_entropy();

    let mut values = Vec::with_capacity(N);
    for _ in 0..N {
        values.push(rng.gen_range(0..1000_000u32))
    }

    let mut insert_perm: Vec<_> = (0..N).collect();
    insert_perm.shuffle(&mut rng);

    let mut remove_perm: Vec<_> = (0..N).collect();
    remove_perm.shuffle(&mut rng);

    // println!("std::collections::BTreeMap");
    // let mut map = std::collections::BTreeMap::new();
    // for &i in insert_perm.iter() {
    //     map.insert(i, values[i]);
    // }
    // for &i in remove_perm.iter() {
    //     assert_eq!(map.remove_entry(&i), Some((i, values[i])));
    // }

    println!("BTree");
    let mut map = BTree::new();
    for &i in insert_perm.iter() {
        map.insert(i, values[i]);
    }
    for &i in remove_perm.iter() {
        assert_eq!(map.remove(&i), Some((i, values[i])));
    }
}

#[test]
pub fn main() {
    // validate_btree_dstruct();
    // validate_ord_btree_dstruct();
    bench_btree_dstruct();
    println!();
    // bench_ord_btree_dstruct();
    // valgrind_btree_dstruct();
}
