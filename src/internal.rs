use core::ptr;

use crate::blocks::{BlockId, Block};

/// Scroll `count` elements starting at `s` to the left `n` times. Return the destination pointer.
///
/// Cost: `O(n)` swaps.
pub unsafe fn scroll_right<T>(s: *mut T, n: usize, count: usize) -> *mut T {
    for i in 0..(n * (count != 0) as usize) {
    	let cur = s.add(i);
    	ptr::swap_nonoverlapping(cur, cur.add(count), 1);
    }
    s.add(n)
}

/// Scroll `count` elements starting at `s` to the right `n` times. Return the destination pointer.
///
/// Cost: `O(n)` swaps.
pub unsafe fn scroll_left<T>(s: *mut T, n: usize, count: usize) -> *mut T {
    for i in 1..=(n * (count != 0) as usize) {
    	let cur = s.sub(i);
    	ptr::swap_nonoverlapping(cur.add(count), cur, 1);
    }
    s.sub(n)
}

/// Merge assuming the following context:
/// ```
/// 	........... LLLLLL RRRRRRRRRRR
///     	epb     excess     epb
/// ```
/// where the L elements are of type `id`, the R elements are of type `!id`, and the ... elements
/// are elements in an internal buffer.
/// Modify the values of `s`, `excess`, and `id` after the merge is complete.
///
/// Cost: `O(n + m)` comparisons and `O(n + m)` moves.
pub unsafe fn merge_up<T, F: FnMut(&T, &T) -> bool>(
	s: &mut *mut T, excess: &mut usize, id: &mut BlockId, epb: usize, less: &mut F,
) {
	#[inline(never)]
	unsafe fn local_merge_up<T, F: FnMut(&T, &T) -> bool>(
		[(a, n), (b, m)]: [(*mut T, usize); 2], dst: *mut T, less: &mut F,
	) -> [usize; 2] {
		let [mut i, mut j] = [0, 0];

		while i != n && j != m {
			let [l, r] = [a.add(i), b.add(j)];
			let right = less(&*r, &*l);
			[i, j] = [i + !right as usize, j + right as usize];
			ptr::swap_nonoverlapping(if right { r } else { l }, dst.add(i + j - 1), 1);
		}

		[n - i, m - j]
	}

	// Perform local merge depending on block id (for stability)
	let [(a, n), (b, m)] = [(s.add(epb), *excess), (s.add(epb + *excess), epb)];
	let [l, r] = if *id == Block::A {
		local_merge_up([(a, n), (b, m)], *s, less)
	} else {
		local_merge_up([(a, n), (b, m)], *s, &mut |x, y| !less(y, x))
	};

	// Rewind buffer if necessary and re-compute values
	scroll_left(b, l, epb);
	*excess = l.max(r);
	*s = b.sub(*excess);
	*id ^= l == 0;
}

/// Merge in-place assuming the following context:
/// ```
/// 	LLLLL RRRRRRR
///		  a      b
/// ```
/// where the L elements are of type `id` and the R elements are of type `!id`.
/// Modify the values of `a` and `id` after the merge is complete.
///
/// Cost: See [`crate::merge::merge_right`].
pub unsafe fn merge_right<'a, T, F: FnMut(&T, &T) -> bool>(
	a: &mut &'a mut [T], b: &'a mut [T], id: &mut BlockId, less: &mut F,
) {
	// Perform local merge and re-compute values
	let [l, r] = if *id == Block::A {
		crate::merge::merge_right([a, b], less)
	} else {
		crate::merge::merge_right([a, b], &mut |x, y| !less(y, x))
	};

	let m = b.len();
	*a = &mut b[m - l.max(r)..];
	*id ^= l == 0;
}
