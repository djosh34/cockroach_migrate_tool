#![allow(dead_code)]

pub trait Predicate<T: ?Sized> {
    fn eval(&self, value: &T) -> bool;
}

pub trait PredicateBooleanExt: Sized {
    fn and<P>(self, other: P) -> AndPredicate<Self, P> {
        AndPredicate {
            left: self,
            right: other,
        }
    }

    fn not(self) -> NotPredicate<Self> {
        NotPredicate { inner: self }
    }
}

impl<T> PredicateBooleanExt for T {}

pub struct AndPredicate<L, R> {
    left: L,
    right: R,
}

impl<L, R> Predicate<str> for AndPredicate<L, R>
where
    L: Predicate<str>,
    R: Predicate<str>,
{
    fn eval(&self, value: &str) -> bool {
        self.left.eval(value) && self.right.eval(value)
    }
}

pub struct NotPredicate<P> {
    inner: P,
}

impl<P> Predicate<str> for NotPredicate<P>
where
    P: Predicate<str>,
{
    fn eval(&self, value: &str) -> bool {
        !self.inner.eval(value)
    }
}

pub mod predicate {
    pub mod str {
        use crate::predicates::Predicate;

        pub struct ContainsPredicate {
            needle: String,
        }

        impl Predicate<str> for ContainsPredicate {
            fn eval(&self, value: &str) -> bool {
                value.contains(&self.needle)
            }
        }

        pub fn contains(needle: impl Into<String>) -> ContainsPredicate {
            ContainsPredicate {
                needle: needle.into(),
            }
        }
    }
}

pub mod prelude {
    pub use crate::predicates::PredicateBooleanExt;
    pub use crate::predicates::predicate;
}
