use crate::merge::{merge_down, merge_up};
use crate::merge::{Merge, MergeUnchecked};

use sort_util::op::move_slice;
use sort_util::Sorted;

/// A collection of contiguous and comparatively distinct elements, called "keys".
pub struct Keys<'a, T> {
    /// The slice that the keys exist in.
    pub inner: &'a mut [T],

    // Constants pertaining to the distribution of keys within this collection.
    buffer_len: usize,
    tags_len: usize,

    // The minimum length at which a run cannot be fully tagged.
    unsortable_left_len: usize,
}

impl<'a, T> Keys<'a, T> {
    /// Establish a new collection of keys over `inner` with a buffer length of `buffer_len`.
    pub fn new(inner: &'a mut [T], buffer_len: usize) -> Self {
        let keys_len = inner.len() - buffer_len;
        let unsortable_left_len = (keys_len + 1) * buffer_len;
        Self { inner, buffer_len, tags_len: keys_len, unsortable_left_len }
    }
}

impl<T> Keys<'_, T> {
    /// Return `true` iff a scrolling block merge with left run `a` is possible.
    pub fn can_scrolling_block_merge(&self, a: &mut [T]) -> bool {
        a.len() < self.unsortable_left_len
    }

    /// Sort this collection of keys.
    pub fn sort_internal_buffer<F: FnMut(&T, &T) -> bool>(&mut self, less: &mut F) {
        self.sort_first(self.inner.len(), less);
    }

    /// Sort the first `len` elements in this collection of keys. We need to sort only the buffer,
    /// by the following invariants:
    /// 1. Our buffer is partitioned to be greater than our tags
    /// 2. Our tags are always sorted
    pub fn sort_first<F: FnMut(&T, &T) -> bool>(&mut self, len: usize, less: &mut F) {
        let tags_len = self.tags_len;
        crate::mini::heap_sort(&mut self.inner[tags_len..len.max(tags_len)], less);
    }

    /// Return slices of the tags portion and the buffer portion of this collection.
    pub fn as_components(&mut self) -> [&mut [T]; 2] {
        let tags_len = self.tags_len;
        let (tags, internal_buffer) = self.inner.split_at_mut(tags_len);
        [tags, internal_buffer]
    }

    /// Return a pointer to the buffer portion of this collection of keys.
    pub fn buffer(&mut self) -> *mut T {
        unsafe { self.inner.as_mut_ptr().add(self.tags_len) }
    }

    fn merge_basic<F: FnMut(&T, &T) -> bool>(
        &mut self, [a, b]: [&mut [T]; 2], less: &mut F,
    ) -> Sorted {
        let shorter_side = usize::min(a.len(), b.len());

        if self.buffer_len < shorter_side {
            return Sorted::Fail;
        }

        if a.len() == shorter_side {
            let dst = a.as_mut_ptr();
            merge_up::<_, true>([unsafe { move_slice::<_, true>(self.buffer(), a) }, b], dst, less);
        } else {
            merge_down::<_, true>([a, unsafe { move_slice::<_, true>(self.buffer(), b) }], less);
        }

        Sorted::Done
    }
}

impl<T> Merge<T> for Keys<'_, T> {
    /// Return `true` iff this key collection has at least one key.
    fn can_merge(&self, _: [&mut [T]; 2]) -> bool {
        // When we call this, we will have collected at least one key. At least, we better have.
        !self.inner.is_empty() || unsafe { core::hint::unreachable_unchecked() }
    }
}

impl<T> MergeUnchecked<T> for Keys<'_, T> {
    /// Perform either an internal regular merge or a block merge.
    ///
    /// Cost: `O(n + m)` comparisons and `O(n + m)` moves, given key collection was done properly.
    fn merge_unchecked<F: FnMut(&T, &T) -> bool>(&mut self, [a, b]: [&mut [T]; 2], less: &mut F) {
        self.merge_basic([a, b], less)
            .or(|| crate::blocks::block_merge(self, [a, b], less));
    }
}
