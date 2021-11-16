use num::{one, zero, Complex, Float, Num};

pub fn bit_reversal<T>(a: &mut [T]) {
    assert!(a.len().is_power_of_two());

    let index_leading_zeros = (a.len() - 1).leading_zeros();
    for i in 0..a.len() {
        let j = i.reverse_bits() >> index_leading_zeros;
        if i < j {
            a.swap(i, j);
        }
    }
}

// pub fn roots_of_unity<T: Float>(n: usize) -> Vec<Complex<T>> {
//     use std::f64::consts::PI;
//
//     match n {
//         0 => vec![],
//         1 => vec![one()],
//         _ => {
//             let mut roots = Vec::with_capacity(n);
//             roots.push(one());
//             roots.push(Complex::from_polar(
//                 one(),
//                 T::from(2.0 * PI).unwrap() / T::from(n).unwrap(),
//             ));
//
//             for _ in 2..n {
//                 roots.push(roots.last().unwrap() * roots[1]);
//             }
//
//             roots
//         }
//     }
// }

pub fn eval_poly<N: Clone + Num>(p: &[N], x: N) -> N {
    let mut out = p.last().unwrap().clone();
    for c in p.iter().rev().skip(1) {
        out = out * x.clone() + c.clone();
    }

    out
}

pub fn fft2<T: Float>(p: &mut [Complex<T>]) {
    assert!(p.len().is_power_of_two());

    bit_reversal(p);

    let mut root_q = Complex::new(-T::one(), zero());
    let mut round = 1;
    while round < p.len() {
        let half_round = round;
        round *= 2;

        for i in (0..p.len()).step_by(round) {
            let (ye, yo) = p[i..].split_at_mut(half_round);

            let mut root: Complex<T> = one();
            for j in 0..half_round {
                let evens = ye[j];
                let odds = root * yo[j];

                ye[j] = evens + odds;
                yo[j] = evens - odds;

                root = root_q * root;
            }
        }

        root_q = root_q.sqrt();
    }
}

pub fn ifft2<T: Float>(p: &mut [Complex<T>]) {
    for x in p.iter_mut() {
        *x = x.conj();
    }
    fft2(p);

    let n = T::from(p.len()).unwrap();
    for x in p.iter_mut() {
        *x = x.conj() / n;
    }
}

#[derive(Copy, Clone)]
pub struct PrintPoly<'a, T>(pub &'a str, pub &'a [T]);

impl<'a, T: std::fmt::Display> std::fmt::Display for PrintPoly<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(x) = ", self.0)?;
        if let Some(precision) = f.precision() {
            match self.1.len() {
                0 => {
                    write!(f, "NaN")?;
                }
                1 => {
                    write!(f, "{1:.*}", precision, self.1[0])?;
                }
                2 => {
                    write!(f, "({:.*})x + {:.0$}", precision, self.1[1], self.1[0])?;
                }
                _ => {
                    for (i, c) in self.1[2..].iter().enumerate().rev() {
                        write!(f, "({:.*})x^{} + ", precision, c, i + 2)?;
                    }
                    write!(f, "({:.*})x + {:.0$}", precision, self.1[1], self.1[0])?;
                }
            }
        } else {
            match self.1.len() {
                0 => {
                    write!(f, "NaN")?;
                }
                1 => {
                    write!(f, "{}", self.1[0])?;
                }
                2 => {
                    write!(f, "({})x + {}", self.1[1], self.1[0])?;
                }
                _ => {
                    for (i, c) in self.1[2..].iter().enumerate().rev() {
                        write!(f, "({})x^{} + ", c, i + 2)?;
                    }
                    write!(f, "({})x + {}", self.1[1], self.1[0])?;
                }
            }
        }

        Ok(())
    }
}

#[allow(dead_code)]
fn test_fft() {
    const P_SRC: [num::Complex<f64>; 16] = [
        num::Complex::new(15.0, 2.0),
        num::Complex::new(7.0, 3.0),
        num::Complex::new(11.0, 0.0),
        num::Complex::new(3.0, 0.0),
        num::Complex::new(13.0, 1.0),
        num::Complex::new(5.0, 0.0),
        num::Complex::new(9.0, 0.0),
        num::Complex::new(1.0, 0.0),
        num::Complex::new(14.0, 0.0),
        num::Complex::new(6.0, 0.0),
        num::Complex::new(10.0, 0.0),
        num::Complex::new(2.0, 0.0),
        num::Complex::new(12.0, 0.0),
        num::Complex::new(4.0, 0.0),
        num::Complex::new(8.0, 0.0),
        num::Complex::new(0.0, 0.0),
    ];
    // bench_prefix_sum_dstruct();
    println!("{}", PrintPoly("p", &P_SRC));

    let mut p = P_SRC;

    fft2(&mut p);
    println!("fft2:");
    for (t, y) in p.iter().enumerate() {
        println!("    p(e ^ ({}*2πi/{})) = {:.3}", t, P_SRC.len(), y);
    }

    ifft2(&mut p);
    println!("ifft2:");

    println!("  {}", PrintPoly("p", &p));
}
