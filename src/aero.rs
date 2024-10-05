use crate::keys::Keys;
use crate::merge::Merge;
use crate::mini::insertion_sort_safe;

/// Perform a merge operation, prioritizing external buffer merges.
///
/// Cost: `O(n)` comparisons and `O(n)` moves if key collection was done properly.
pub fn merge_regular<T, F: FnMut(&T, &T) -> bool>(
	[a, b]: [&mut [T]; 2], ext: &mut [T], keys: &mut Keys<T>, less: &mut F,
) {
	ext.merge([a, b], less)
		// Use keys only if external merge isn't possible
		.or(|| keys.merge([a, b], less));
}

// Sort `v` using a merge strategy `merge`.
fn sort_with_merge_strategy<T, F: FnMut(&T, &T) -> bool>(
	v: &mut [T], less: &mut F, mut merge: impl FnMut([&mut [T]; 2], &mut F),
) {
	let n = v.len();

	// `0 <= i <= factor <= n <= isize::MAX` (`isize::MAX` is the maximum slice length), so we can
	// fit `n * i <= isize::MAX * isize::MAX < 2^126` in a u128.
	let factor = (1 << sort_util::op::log2_ceil(n / 16)) as u128;
	let bound = |i| (n as u128 * i / factor) as usize;

	// Merge sort loop
	let mut right = 0;
	let mut mid;
	for i in 1..=factor {
		[mid, right] = [right, bound(i)];
		insertion_sort_safe(&mut v[mid..right], less);

		for k in 1..=i.trailing_zeros() {
			let left = bound(i - (1 << k));
			let (a, b) = v[left..right].split_at_mut(mid - left);
			merge([a, b], less);
			mid = left;
		}
	}
}

// Sort `v` using `ext` as an external buffer and `keys`.
fn sort<T, F: FnMut(&T, &T) -> bool>(v: &mut [T], ext: &mut [T], keys: &mut Keys<T>, less: &mut F) {
	sort_with_merge_strategy(v, less, |[a, b], less| merge_regular([a, b], ext, keys, less));
}

// Sort `v` with in-place merging.
fn sort_lazy<T, F: FnMut(&T, &T) -> bool>(v: &mut [T], less: &mut F) {
	sort_with_merge_strategy(v, less, |[a, b], less| crate::merge::merge_in_place([a, b], less) );
}

// Sort `v` with `ext` as an external buffer, assuming we can use it for every merge.
fn sort_easy<T, F: FnMut(&T, &T) -> bool>(v: &mut [T], ext: &mut [T], less: &mut F) {
	sort_with_merge_strategy(v, less, |[a, b], less| { ext.merge([a, b], less); });
}

/// Sort `v` with `ext` as an external buffer.
///
/// Cost: `O(n log n)` comparisons and `O(n log n)` moves.
pub fn sort_full<T, F: FnMut(&T, &T) -> bool>(v: &mut [T], ext: &mut [T], less: &mut F) {
	let n = v.len();

	// Use insertion sort for small arrays
	if n <= 64 {
		return insertion_sort_safe(v, less);
	}

	// If our buffer is sufficiently large, we can be sure that it can perform every merge
	if ext.len() >= n / 2 {
		return sort_easy(v, ext, less);
	}

	// Collect keys and sort
    let mut state = crate::state::collect_keys(v, less);

    match state.keys.inner.len() {
   		// If the slice turns out to contain 1 value, we are done
    	1 => (),

    	// If the slice turns out to contain 12 or less values, just use rotation-based merging
    	cnt if 2 <= cnt && cnt <= 12 => sort_lazy(v, less),

    	// Perform normal block merge sort
    	_ => {
    		sort(state.task, ext, &mut state.keys, less);
    		state.restore_by(less);
    	}
    }
}
