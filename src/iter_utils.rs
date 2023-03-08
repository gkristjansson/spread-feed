use std::{cmp::Ordering, iter::Peekable};

use futures::future::Either;

/// An iterator that will yield the smaller or larger of two iterators, depending on the on the `cmp` argument.
/// This is useful for merging two sorted iterators into a single sorted iterator.
///
/// # Example
/// ```
/// use std::vec;
/// use iter_utils::OrderedChainExt;
///
/// let a = vec![1, 3, 5];
/// let b = vec![2, 4, 6];
/// let it = a.iter().order_chained(b.iter(), Ordering::Less);
/// let expected = vec![1, 2, 3, 4, 5, 6];
///  panic!("Testing")
///  assert_eq!(it.cmp(expected.iter()), Ordering::Equal);
/// ```
pub struct OrderedChain<A: Iterator, B: Iterator> {
    a: Peekable<A>,
    b: Peekable<B>,
    cmp: Ordering,
}

impl<A: Iterator, B: Iterator> OrderedChain<A, B> {
    pub fn new(a: A, b: B, cmp: Ordering) -> Self {
        assert_ne!(cmp, Ordering::Equal);
        OrderedChain {
            a: a.peekable(),
            b: b.peekable(),
            cmp,
        }
    }
}

impl<A, B> Iterator for OrderedChain<A, B>
where
    A: Iterator,
    B: Iterator<Item = A::Item>,
    A::Item: Ord,
{
    type Item = Either<A::Item, B::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        let a = self.a.peek();
        let b = self.b.peek();

        match (a, b) {
            (Some(a), Some(b)) => {
                if a.cmp(b) == self.cmp {
                    self.a.next().map(Either::Left)
                } else {
                    self.b.next().map(Either::Right)
                }
            }
            (Some(_), None) => self.a.next().map(Either::Left),
            (None, Some(_)) => self.b.next().map(Either::Right),
            (None, None) => None,
        }
    }
}

pub trait OrderedChainExt: Iterator {
    fn ordered_chain<I>(self, other: I, cmp: Ordering) -> OrderedChain<Self, I>
    where
        I: Iterator<Item = Self::Item>,
        Self: Sized,
    {
        OrderedChain::new(self, other, cmp)
    }
}

impl<T: ?Sized> OrderedChainExt for T where T: Iterator {}
#[cfg(test)]
mod test {
    use std::cmp::Ordering;

    use crate::iter_utils::OrderedChain;

    #[test]
    fn test_same_length_and_less_then() {
        let a = vec![1, 3, 5];
        let b = vec![2, 4, 6];

        let it = OrderedChain::new(a.iter(), b.iter(), Ordering::Less).map(|f| f.into_inner());
        let expected = vec![1, 2, 3, 4, 5, 6];

        assert_eq!(it.cmp(expected.iter()), Ordering::Equal);
    }


    #[test]
    fn test_a_shorter_length_and_less_then() {
        let a = vec![1, 3];
        let b = vec![2, 4, 5, 6];

        let it = OrderedChain::new(a.iter(), b.iter(), Ordering::Less).map(|f| f.into_inner());
        let expected = vec![1, 2, 3, 4, 5, 6];

        assert_eq!(it.cmp(expected.iter()), Ordering::Equal);
    }

    #[test]
    fn test_b_shorter_length_and_less_then() {
        let a = vec![1, 3, 5, 6];
        let b = vec![2, 4];

        let it = OrderedChain::new(a.iter(), b.iter(), Ordering::Less).map(|f| f.into_inner());
        let expected = vec![1, 2, 3, 4, 5, 6];

        assert_eq!(it.cmp(expected.iter()), Ordering::Equal);
    }


    #[test]
    fn test_same_length_and_greater_then() {
        let a = vec![5, 3, 1];
        let b = vec![6, 4, 2];

        let it = OrderedChain::new(a.iter(), b.iter(), Ordering::Greater).map(|f| f.into_inner());
        let expected = vec![6, 5, 4, 3, 2, 1];

        assert_eq!(it.cmp(expected.iter()), Ordering::Equal);
    }

    #[test]
    fn test_a_shorter_length_and_greater_then() {
        let a = vec![5, 3];
        let b = vec![6, 4, 2, 1];

        let it = OrderedChain::new(a.iter(), b.iter(), Ordering::Greater).map(|f| f.into_inner());
        let expected = vec![6, 5, 4, 3, 2, 1];

        assert_eq!(it.cmp(expected.iter()), Ordering::Equal);
    }

    #[test]
    fn test_b_shorter_length_and_greater_then() {
        let a = vec![5, 3, 2, 1];
        let b = vec![6, 4];

        let it = OrderedChain::new(a.iter(), b.iter(), Ordering::Greater).map(|f| f.into_inner());
        let expected = vec![6, 5, 4, 3, 2, 1];

        assert_eq!(it.cmp(expected.iter()), Ordering::Equal);
    }


}
