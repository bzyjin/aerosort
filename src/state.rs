use crate::keys::Keys;

use sort_util::op::{self, lower_bound, search_unique};
use sort_util::{op::Found, GenerateSlice, RawMut};

/// A state storing properties of a current key collection originating leftwards.
pub struct LeftCollectState<T> {
    location: *mut T,
    keys: usize,
}

impl<T> LeftCollectState<T> {
    /// Initialize a new key collection at `location` of length `keys`, assuming it is pre-sorted.
    pub fn new(location: *mut T, keys: usize) -> Self {
        Self { location, keys }
    }

    unsafe fn insert<F: FnMut(&T, &T) -> bool>(&mut self, key: *mut T, less: &mut F) {
        let Found(false, index) = search_unique(self.location, self.keys, &*key, less) else {
            return;
        };

        let shift = key.offset_from(self.location) as usize - self.keys;
        op::rotate(self.location, self.keys + shift, self.keys);
        self.location = self.location.add(shift);

        op::insert_left(key, self.keys - index);
        self.keys += 1;
    }

    /// Perform a complete key collection of `v`, scanning to the right. Abort collection once
    /// `limit` keys are collected. Return the number of keys collected.
    pub fn scan<F: FnMut(&T, &T) -> bool>(
        &mut self, v: &mut [T], limit: usize, less: &mut F,
    ) -> usize {
        let s = v.as_mut_ptr();
        let initial_keys = self.keys;

        for i in 0..v.len() {
            // Try to insert the current key
            unsafe { self.insert(s.add(i), less); }

            // Break early if we reach the desired number of keys
            if self.keys == limit {
                break;
            }
        }

        self.keys - initial_keys
    }

    /// Move the key collection to the left of `v`, ensuring it is sorted ascending. Return a union
    /// state with keys that have an internal buffer of length `buffer_len`.
    pub fn into_union_state<'a>(self, v: &mut [T], buffer_len: usize) -> UnionState<'a, T> {
        let (s, n) = v.raw_mut();
        unsafe {
            let shift = self.location.offset_from(s) as usize;

            // Move our collection to the left of `v` and rotate the interior to be sorted
            op::rotate(s, shift + self.keys, shift);

            let (internal_buffer, task) = s.crop(0..n).split_at_mut(self.keys);
            UnionState {
                align: KeysAlignment::Left,
                keys: Keys::new(internal_buffer, buffer_len),
                task,
            }
        }
    }
}

enum KeysAlignment {
    Left,
    #[allow(unused)]
    Right,
}

/// A state that holds information for sorting a slice.
pub struct UnionState<'a, T> {
    align: KeysAlignment,

    /// The formed collection of keys.
    pub keys: Keys<'a, T>,

    /// The slice to be sorted.
    pub task: &'a mut [T],
}

impl<'a, T> UnionState<'a, T> {
    /// Restore all keys into the slice, completing the sorting operation.
    ///
    /// Cost: `O(sqrt n * log n)` comparisons and `O(n)` moves.
    pub fn restore_by<F: FnMut(&T, &T) -> bool>(&mut self, less: &mut F) {
        use crate::merge::{merge_left, merge_right};

        self.keys.sort_internal_buffer(less);
        match self.align {
            KeysAlignment::Left => { merge_right([self.keys.inner, self.task], less); }
            KeysAlignment::Right => merge_left([self.task, self.keys.inner], less)
        }
    }
}

/// Collect keys from `v` and return a [`UnionState`] representing the created state.
pub fn collect_keys<'a, T, F: FnMut(&T, &T) -> bool>(
    v: &'a mut [T], less: &mut F,
) -> UnionState<'a, T> {
    let n = v.len();

    // Collecting `2 sqrt n` keys reduces total comparisons by ~1% with large `n`, but results in a
    // more expensive final redistribution, so we might as well not worry about that.
    let mut k = lower_bound::binary(n, |i| i * i < 2 * n);
    k -= (k * k != 2 * n) as usize;    // `keys == (2 * n).isqrt()`

    // Collect up to `k` keys
    let mut collection = LeftCollectState::new(v.as_mut_ptr(), 1);
    collection.scan(&mut v[1..], k, less);
    k = collection.keys;

    // We can expand our buffer as long as we have enough keys (an approximation is used here)
    let buffer_len = k - lower_bound::binary(k / 2, |len| len < (n - k) / 2 / (k - len));

    // Move our collection to the far left
    collection.into_union_state(v, buffer_len)
}
