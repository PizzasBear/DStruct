#![allow(dead_code)]

use std::ops;

#[derive(Default, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Complex64 {
    pub real: f32,
    pub imag: f32,
}

impl Complex64 {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    #[inline]
    pub const fn new(real: f32, imag: f32) -> Self {
        Self { real, imag }
    }

    #[inline]
    pub fn square(self) -> Self {
        // (a+bi)^2 = a*a + 2abi - b*b
        Self::new(
            self.real * self.real - self.imag * self.imag,
            2.0 * self.real * self.imag,
        )
    }

    pub fn exp(self) -> Self {
        let mul = self.real.exp();
        let (s, c) = self.imag.sin_cos();
        Self::new(c * mul, s * mul)
    }

    pub fn ln(self) -> Self {
        Self::new(
            (self.real * self.real + self.imag * self.imag).ln() / 2.0,
            f32::atan2(self.imag, self.real),
        )
    }

    // Calculates `log_other(self)`.
    //
    // # Warning
    /// This simplifies internally from `log_b x` to `ln x / ln b`, so for repeated use of either
    /// the base or the parameter, it would be perferable to precalculate its `ln` and compute
    /// `ln x / ln b` directly.
    #[inline]
    pub fn log(self, other: Self) -> Self {
        // log_a x = ln x / ln a
        self.ln() / other.ln()
    }

    /// Calculates `self ^ other`.
    ///
    /// # Warning
    /// This simplifies internally from `a^b` to `e^(ln a * b)`, so for repeated use of a base, it
    /// would be perferable to precalculate `ln a` and compute `e^(ln a * b)` directly.
    #[inline]
    pub fn pow(self, other: Self) -> Self {
        (self.ln() * other).exp()
    }
}

impl ops::Add for Complex64 {
    type Output = Self;

    #[inline]
    fn add(self, other: Self) -> Self {
        Self::new(self.real + other.real, self.imag + other.imag)
    }
}

impl ops::AddAssign for Complex64 {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        self.real += other.real;
        self.imag += other.imag;
    }
}

impl ops::Neg for Complex64 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self::new(-self.real, -self.imag)
    }
}

impl ops::Sub for Complex64 {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self::new(self.real - other.real, self.imag - other.imag)
    }
}

impl ops::SubAssign for Complex64 {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.real -= other.real;
        self.imag -= other.imag;
    }
}

impl ops::Mul for Complex64 {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        // (a + bi)(c + di) = ac - bd + (ad + bc)i
        Self::new(
            self.real * other.real - self.imag * other.imag,
            self.real * other.imag + self.imag * other.real,
        )
    }
}

impl ops::MulAssign for Complex64 {
    fn mul_assign(&mut self, other: Self) {
        let self_real = self.real;
        self.real = self_real * other.real - self.imag * other.imag;
        self.imag = self_real * other.imag + self.imag * other.real;
    }
}

impl ops::Div for Complex64 {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        // (a+bi)/(c+di) = (a+bi)(c-di)/((c+di)(c-di)) = (ac+bd + (bc-ad)i)/(c*c + d*d)
        let div = other.real * other.real + other.imag * other.imag;
        Self::new(
            (self.real * other.real + self.imag * other.imag) / div,
            (self.imag * other.real - self.real * other.imag) / div,
        )
    }
}

impl ops::DivAssign for Complex64 {
    fn div_assign(&mut self, other: Self) {
        let div = other.real * other.real + other.imag * other.imag;
        let self_real = self.real;
        self.real = (self_real * other.real + self.imag * other.imag) / div;
        self.imag = (self.imag * other.real - self_real * other.imag) / div;
    }
}
