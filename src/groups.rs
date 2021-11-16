pub trait Magma {
    type Elem: Clone;

    fn add(&self, lhs: Self::Elem, rhs: Self::Elem) -> Self::Elem;
}

pub trait Associativity: Magma {}
pub trait Commutativity: Magma {}
pub trait Identity: Magma {
    /// Identity
    fn id(&self) -> Self::Elem;
}
pub trait Invertibillity: Magma {
    // Invertibillity
    fn inv(&self, x: Self::Elem) -> Self::Elem;

    #[inline]
    fn sub(&self, lhs: Self::Elem, rhs: Self::Elem) -> Self::Elem {
        self.add(lhs, self.inv(rhs))
    }
}

pub trait Monoid: Magma + Associativity + Identity {}
impl<M: Magma + Associativity + Identity> Monoid for M {}

pub trait CommutativeMonoid: Monoid + Commutativity {}
impl<CM: Monoid + Commutativity> CommutativeMonoid for CM {}

pub trait Group: Magma + Associativity + Identity + Invertibillity {}
impl<G: Magma + Associativity + Identity + Invertibillity> Group for G {}

pub trait AbelianGroup: Group + Commutativity {}
impl<AG: Group + Commutativity> AbelianGroup for AG {}

#[derive(Clone, Copy, Debug)]
pub struct NumAdditiveGroups<T>(std::marker::PhantomData<T>);

impl<T> Default for NumAdditiveGroups<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> NumAdditiveGroups<T> {
    pub const fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: num::Num + Clone> Magma for NumAdditiveGroups<T> {
    type Elem = T;

    #[inline]
    fn add(&self, lhs: T, rhs: T) -> T {
        lhs + rhs
    }
}
impl<T: num::Num + Clone> Identity for NumAdditiveGroups<T> {
    #[inline]
    fn id(&self) -> T {
        T::zero()
    }
}
impl<T: num::Num + num::Signed + Clone> Invertibillity for NumAdditiveGroups<T> {
    #[inline]
    fn inv(&self, x: T) -> T {
        -x
    }

    #[inline]
    fn sub(&self, lhs: T, rhs: T) -> T {
        lhs - rhs
    }
}
impl<T: num::Num + Clone> Associativity for NumAdditiveGroups<T> {}
impl<T: num::Num + Clone> Commutativity for NumAdditiveGroups<T> {}
