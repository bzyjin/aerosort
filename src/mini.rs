use core::ptr;

use sort_util::RawMut;

/// Sort `v` with a guarded insertion sort.
///
/// Cost: `O(n^2)` comparisons and `O(n^2)` moves.
#[inline(never)]
pub fn insertion_sort_safe<T, F: FnMut(&T, &T) -> bool>(v: &mut [T], less: &mut F) {
    use core::mem::ManuallyDrop;

    // Represents the slot created on each insertion
    struct Slot<T>(ManuallyDrop<T>, *mut T, usize);

    impl<T> core::ops::Drop for Slot<T> {
        fn drop(&mut self) {
            unsafe { ptr::copy_nonoverlapping(&*self.0, self.1.add(self.2), 1); }
        }
    }

    let (s, n) = v.raw_mut();

    for i in 1..n {
        unsafe {
            let mut slot = Slot(ManuallyDrop::new(s.add(i).read()), s, i);

            while slot.2 != 0 && less(&slot.0, &*s.add(slot.2 - 1)) {
                slot.2 -= 1;
                ptr::copy_nonoverlapping(s.add(slot.2), s.add(slot.2 + 1), 1);
            }
        }
    }
}

/// Sort `v` with heap sort.
///
/// Cost: `O(n log n)` comparisons and `O(n log n)` moves.
#[inline(never)]
pub fn heap_sort<T, F: FnMut(&T, &T) -> bool>(v: &mut [T], less: &mut F) {
    let n = v.len();

    // Source: https://github.com/Voultapher/tiny-sort-rs/blob/main/src/unstable.rs
    unsafe {
        (0..n / 2).rev().for_each(|i| sift_down(v, i, less));

        for i in (1..n).rev() {
            v.swap_unchecked(0, i);
            sift_down(&mut v[..i], 0, less);
        }
    }
}

#[inline(never)]
unsafe fn sift_down<T, F: FnMut(&T, &T) -> bool>(v: &mut [T], mut root: usize, less: &mut F) {
    let (s, n) = v.raw_mut();

    loop {
        let mut child = 2 * root + 1;
        if child >= n {
            return;
        }

        if child + 1 < n {
            child += less(&*s.add(child), &*s.add(child + 1)) as usize;
        }

        if !less(&*s.add(root), &*s.add(child)) {
            return;
        }

        ptr::swap(s.add(child), s.add(root));
        root = child;
    }
}
