#![no_std]
#![deny(missing_docs)]

//! aerosort is a sorting library. It is comparison-based, stable, and in-place by default. The
//! following interface is provided:
//!
//! | Family         | Heap allocation (elements) |
//! |----------------|----------------------------|
//! | [`sort`]       | none                       |
//! | [`sort_with`]  | given (variable)           |
//!
//! To sort using a comparator, use the `_by` extension and pass a comparison function e.g.
//! [`sort_by`]`(&mut v, cmp)`. This allows you to sort descending and into other desired patterns.
//!
//! To sort by key, use the `_by_key` interface and pass a mapping e.g. [`sort_by_key`]
//! `(&mut v, f)`. This will sort ascending by key (lowest keys first).
//!
//! The worst-case time complexity is always `O(n log n)` across all external space sizes.

mod aero;
mod blocks;
mod internal;
mod keys;
mod merge;
mod mini;

#[cfg(not(feature = "internal"))]
mod state;

#[cfg(feature = "internal")]
/// Module that exposes the key collection process.
pub mod state;

#[cfg(feature = "internal")]
pub use aero::merge_regular;

use core::cmp::Ordering;

use sort_util::buffer::{self, AsSliceMut};

/// Sort `v`.
#[inline(always)]
pub fn sort<T: Ord>(v: &mut [T]) {
    sort_by(v, &mut T::cmp)
}

/// Sort `v` with a comparison function `cmp`.
#[inline(always)]
pub fn sort_by<T>(v: &mut [T], cmp: impl FnMut(&T, &T) -> Ordering) {
    sort_with_by(v, buffer::create(0), cmp)
}

/// Sort `v` with a mapping `f` from elements to keys.
#[inline(always)]
pub fn sort_by_key<T, K: Ord>(v: &mut [T], f: impl FnMut(&T) -> K) {
    sort_with_by_key(v, buffer::create(0), f)
}

/// Sort `v` with an external buffer `ext`.
#[inline(always)]
pub fn sort_with<T: Ord>(v: &mut [T], ext: impl AsSliceMut<T>) {
    sort_with_by(v, ext, &mut T::cmp)
}

/// Sort `v` with an external buffer `ext` and a comparison function `cmp`.
#[inline(always)]
pub fn sort_with_by<T>(
    v: &mut [T], mut ext: impl AsSliceMut<T>, mut cmp: impl FnMut(&T, &T) -> Ordering,
) {
    sort_general(v, ext.as_slice_mut(), &mut |x, y| cmp(x, y) == Ordering::Less)
}

/// Sort `v` with an external buffer `ext` and a mapping `f` from elements to keys.
#[inline(always)]
pub fn sort_with_by_key<T, K: Ord>(
    v: &mut [T], mut ext: impl AsSliceMut<T>, mut f: impl FnMut(&T) -> K,
) {
    sort_general(v, ext.as_slice_mut(), &mut |x, y| f(x).lt(&f(y)))
}

#[inline(always)]
fn sort_general<T, F: FnMut(&T, &T) -> bool>(v: &mut [T], ext: &mut [T], less: &mut F) {
    // Skip zero-sized types
    if core::mem::size_of::<T>() != 0 {
        aero::sort_full(v, ext, less);
    }
}
