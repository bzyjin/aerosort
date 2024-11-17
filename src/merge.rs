use sort_util::op::{move_slice, rotate, search, write};
use sort_util::{RawMut, Sorted};

/// A trait for merging two sorted slices under the assumption that the operation is possible.
pub trait MergeUnchecked<T> {
    fn merge_unchecked<F: FnMut(&T, &T) -> bool>(&mut self, pair: [&mut [T]; 2], less: &mut F);
}

/// A trait for merging two sorted slices with a safety check.
pub trait Merge<T>: MergeUnchecked<T> {
    fn can_merge(&self, pair: [&mut [T]; 2]) -> bool;

    /// Try to merge `a` and `b` and return [`Sorted::Done`]. Otherwise, return [`Sorted::Fail`].
    fn merge<F: FnMut(&T, &T) -> bool>(&mut self, [a, b]: [&mut [T]; 2], less: &mut F) -> Sorted {
        if !self.can_merge([a, b]) {
            Sorted::Fail
        } else {
            self.merge_unchecked([a, b], less);
            Sorted::Done
        }
    }
}

impl<T> Merge<T> for [T] {
    /// Return true iff we can copy either `a` or `b` into `self`.
    fn can_merge(&self, [a, b]: [&mut [T]; 2]) -> bool {
        self.len() >= a.len() || self.len() >= b.len()
    }
}

impl<T> MergeUnchecked<T> for [T] {
    /// Copy either `a` or `b` into `self` and merge.
    ///
    /// Cost: `O(n + m)` comparisons and `O(n + m)` moves.
    fn merge_unchecked<F: FnMut(&T, &T) -> bool>(&mut self, [a, b]: [&mut [T]; 2], less: &mut F) {
        unsafe {
            if a.len() <= b.len() {
                merge_up::<_, false>([move_slice::<_, false>(self.as_mut_ptr(), a), b], less);
            } else {
                merge_down::<_, false>([a, move_slice::<_, false>(self.as_mut_ptr(), b)], less);
            }
        }
    }
}

/// Merge `a` and `b` starting at `dst` and building the result rightwards.
///
/// Cost: `O(n + m)` comparisons and `O(n + m)` moves.
pub fn merge_up<T, const S: bool>([a, b]: [&mut [T]; 2], less: &mut impl FnMut(&T, &T) -> bool) {
    // Represents the gap to the left of `b`
    struct Gap<T, const S: bool>(*mut T, usize, *mut T, usize, usize);

    impl<T, const S: bool> core::ops::Drop for Gap<T, S> {
        fn drop(&mut self) {
            unsafe {
                write::<_, S>(self.0.add(self.3), self.2.add(self.3 + self.4), self.1 - self.3);
            }
        }
    }

    let [(a, n), (b, m)] = [a, b].map(RawMut::raw_mut);

    unsafe {
        let dst = b.sub(n);
        let mut gap = Gap::<T, S>(a, n, dst, 0, 0);

        while gap.3 != n && gap.4 != m {
            let [l, r] = [a.add(gap.3), b.add(gap.4)];
            let right = less(&*r, &*l);
            [gap.3, gap.4] = [gap.3 + !right as usize, gap.4 + right as usize];
            write::<_, S>(if right { r } else { l }, dst.add(gap.3 + gap.4 - 1), 1);
        }
    }
}

/// Merge `a` and `b` with the gap to the right of `a` and building the result leftwards.
///
/// Cost: `O(n + m)` comparisons and `O(n + m)` moves.
pub fn merge_down<T, const S: bool>([a, b]: [&mut [T]; 2], less: &mut impl FnMut(&T, &T) -> bool) {
    // Represents the gap to the right of `a`
    struct Gap<T, const S: bool>(*mut T, *mut T, usize, usize);

    impl<T, const S: bool> core::ops::Drop for Gap<T, S> {
        fn drop(&mut self) {
            unsafe { write::<_, S>(self.1, self.0.add(self.2), self.3); }
        }
    }

    let [(a, n), (b, m)] = [a, b].map(RawMut::raw_mut);

    unsafe {
        let mut gap = Gap::<T, S>(a, b, n, m);

        while gap.2 != 0 && gap.3 != 0 {
            let [l, r] = [a.add(gap.2 - 1), b.add(gap.3 - 1)];
            let left = less(&*r, &*l);
            [gap.2, gap.3] = [gap.2 - left as usize, gap.3 - !left as usize];
            write::<_, S>(if left { l } else { r }, a.add(gap.2 + gap.3), 1);
        }
    }
}

/// Merge `a` and `b` by rotating `b` into `a`, assuming `b.len() <= a.len()`.
///
/// Cost: `O(m log n/m + m)` comparisons and `O(n + m^2)` moves.
pub fn merge_left<T, F: FnMut(&T, &T) -> bool>([a, b]: [&mut [T]; 2], less: &mut F) {
    let [(a, mut n), (_, mut m)] = [a, b].map(RawMut::raw_mut);

    unsafe {
        while m != 0 {
            let len = n - search::binary(a, n, a.add(n + m - 1), &mut |x, y| !less(y, x));
            rotate(a.add(n - len), len + m, len);
            n -= len;

            if n == 0 {
                break;
            }

            m = search::binary(a.add(n), m, a.add(n - 1), less);
        }
    }
}

/// Merge `a` and `b` by rotating `a` into `b`, assuming `a.len() <= b.len()`. Return the lengths of
/// the tails of `a` and `b`.
///
/// Cost: `O(n log m/n + n)` comparisons and `O(m + n^2)` moves.
pub fn merge_right<T, F: FnMut(&T, &T) -> bool>([a, b]: [&mut [T]; 2], less: &mut F) -> [usize; 2] {
    let [(a, mut n), (_, mut m)] = [a, b].map(RawMut::raw_mut);

    unsafe {
        let r = a.add(n + m);

        while n != 0 {
            let index = search::binary(r.sub(m), m, r.sub(m + n), less);
            rotate(r.sub(m + n), n + index, n);
            m -= index;

            if m == 0 {
                break;
            }

            n -= search::binary(r.sub(m + n), n, r.sub(m), &mut |x, y| !less(y, x));
        }

        [n, m]
    }
}

/// Merge `a` and `b` in-place using rotations.
///
/// Cost: See [`merge_left`] and [`merge_right`].
pub fn merge_in_place<T, F: FnMut(&T, &T) -> bool>([a, b]: [&mut [T]; 2], less: &mut F) {
    if a.len() <= b.len() {
        merge_right([a, b], less);
    } else {
        merge_left([a, b], less);
    }
}
