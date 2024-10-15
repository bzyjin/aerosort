use core::ops::Range;
use core::ptr;

use crate::internal::{self, scroll_right};
use crate::keys::Keys;
use crate::merge::merge_up;

use sort_util::{GenerateSlice, RawMut, Sorted::{self, *}};

/// An alias for `bool`, where `true` indicates an A-block and `false` indicates a B-block.
pub type BlockId = bool;

/// Holds constants associating A-blocks and B-blocks with values (see [`BlockId`]).
pub struct Block;
impl Block {
    pub const A: BlockId = true;
    pub const B: BlockId = false;
}

/// Merge `a` and `b` using a scrolling block merge whenever applicable, or an in-place block merge.
/// Return whether or not a merge was done.
pub fn block_merge<T, F: FnMut(&T, &T) -> bool>(
    keys: &mut Keys<T>, [a, b]: [&mut [T]; 2], less: &mut F,
) -> Sorted {
    unsafe {
        scrolling_block_merge(keys, [a, b], less)
            .or(|| in_place_block_merge(keys, [a, b], less))
    }
}

// We prefer dynamic dispatch for our block merge loop to reduce compile times, binary size, and
// simplify the implementation. This hurts performance, but not by much, as we will call `on_drop`
// and `init_min` each `O(sqrt n)` times in an overall `O(n)` merge operation.
struct MergeContext<'a, T, F: FnMut(&T, &T) -> bool> {
    constants: (*mut T, *mut T, usize, usize, usize),

    // On dropping a block, call this function with the following parameters:
    // (current block type, excess block type, number of B blocks, comparison function)
    on_drop: &'a mut dyn FnMut(BlockId, &mut BlockId, usize, &mut F),

    // Until we drop our first B-block, call this function with the number of blocks dropped, and
    // the returned value is the index of the minimum A-block.
    init_min: &'a mut dyn FnMut(usize) -> usize,
}

impl<'a, T, F: FnMut(&T, &T) -> bool> MergeContext<'a, T, F> {
    // Perform a full block merge according to stored context. Return the block type of the tail
    // group of elements (final run of consecutive A/B-elements).
    unsafe fn merge_on(self, range: Range<usize>, less: &mut F) -> BlockId {
        let [cnt_a, cnt_b] = [self.constants.2, self.constants.3];
        MergeState { context: self, pid: true, i: 0, cnt_a, cnt_b, ai: 0 }
            .merge_on(range, less)
    }
}

// Holds the current state of a block merge.
struct MergeState<'a, T, F: FnMut(&T, &T) -> bool> {
    context: MergeContext<'a, T, F>,
    pid: BlockId,
    i: usize,
    cnt_a: usize,
    cnt_b: usize,
    ai: usize,
}

impl<'a, T, F: FnMut(&T, &T) -> bool> MergeState<'a, T, F> {
    // Drop the next block.
    #[inline(never)]
    unsafe fn drop_once(&mut self, less: &mut F) -> BlockId {
        let (s, tags, na, _, epb) = self.context.constants;
        let MergeState { i, cnt_a, cnt_b, ai: min_a, .. } = *self;

        // Choose which block to drop (between first B-block and min. A-block)
        let bi = i + cnt_a;
        let id = cnt_b == 0 || cnt_a != 0 && !less(&*s.add(bi * epb), &*s.add(min_a * epb));
        let src = if id == Block::A { min_a } else { bi };
        let step_a = id as usize;

        // Drop the next block
        let dst = s.add(i * epb);
        ptr::swap_nonoverlapping(dst, s.add(src * epb), (src != i) as usize * epb);
        ptr::swap_nonoverlapping(tags.add(na - cnt_a), dst.add(step_a), step_a);
        [self.cnt_a, self.cnt_b] = [self.cnt_a - step_a, self.cnt_b + step_a - 1];

        // Handle new block
        (self.context.on_drop)(id, &mut self.pid, self.cnt_b, less);
        id
    }

    // Perform a full roll/drop + merge loop.
    #[inline(never)]
    unsafe fn merge_on(mut self, range: Range<usize>, less: &mut F) -> BlockId {
        macro_rules! select_while {
            ($cond:expr => $min:expr) => {
                while $cond && (self.pid || self.cnt_a != 0) {
                    let bi = self.i + self.cnt_a;
                    let is_a = self.drop_once(less) == Block::A;
                    self.ai = if is_a { $min } else if self.ai == self.i { bi } else { self.ai };
                    self.i += 1;
                }
            }
        }

        self.i = range.start;
        self.ai = (self.context.init_min)(self.i);
        let (s, _, na, nb, epb) = self.context.constants;

        // Select the minimum block on the range `start..start + count`
        let min_block = |start, count, epb, less: &mut F|
            (start + 1..start + count).fold(start, |res, i| {
                if less(&*s.add(i * epb + 1), &*s.add(res * epb + 1)) { i } else { res }
            });

        select_while!{nb == self.cnt_b => (self.context.init_min)(self.i + 1)};
        select_while!{self.i < na => min_block(na, self.cnt_a + self.i + 1 - na, epb, less)};
        select_while!{self.i < range.end => min_block(self.i + 1, self.cnt_a, epb, less)};

        self.pid
    }
}

// Perform a block merge with a scrolling buffer.
//
// Cost: `O(n)` comparisons and `O(n)` moves.
unsafe fn scrolling_block_merge<T, F: FnMut(&T, &T) -> bool>(
    keys: &mut Keys<T>, [a, b]: [&mut [T]; 2], less: &mut F,
) -> Sorted {
    if !keys.can_scrolling_block_merge(a) {
        return Fail;
    }

    // `tags` points to the start of the tags portion of our key collection
    // `buf_origin` points to the start of the internal buffer portion of our key collection
    // `na` and `nb` count the number of A and B blocks
    // `qa` and `qb` are the size of the undersized A and B blocks
    let [(tags, _), (buf_origin, epb)] = keys.as_components().map(RawMut::raw_mut);
    let [(a, n), (b, m)] = [a, b].map(RawMut::raw_mut);
    let [na, nb, qa, qb] = [n / epb, m / epb, n % epb, m % epb];
    let s = a.add(qa);

    // Tag and "shift" blocks
    let na = na - 1;
    (0..na).for_each(|i| ptr::swap(tags.add(i), s.add(i * epb + 1)));
    ptr::swap_nonoverlapping(s, buf_origin, epb);
    internal::scroll_left(s, qa, epb);
    ptr::swap_nonoverlapping(buf_origin, s.add(na * epb), (na != 0) as usize * epb);

    // Complete the block merge, excluding the tail elements (`qb`)
    let (mut buf, mut excess) = (a, qa);
    if (MergeContext {
        constants: (s, tags, na, nb, epb),
        on_drop: &mut |id, pid, nb, less| {
            if id == *pid {
                buf = scroll_right(buf, (*pid == Block::B || nb != 0) as usize * excess, epb);
                excess = epb;
            } else {
                internal::merge_up(&mut buf, &mut excess, pid, epb, less);
            }
        },
        init_min: &mut |_| na,
    }).merge_on(1..na + nb + 1, less) == Block::B {
        // The rest of the elements are from B; after merging our A-block up, we are done
        merge_up::<_, true>([buf_origin.crop(0..epb), buf.add(epb).to(b.add(m))], less);
    } else {
        // The rest of the elements are from A; first merge B-block up
        let [(a, n), (b, m)] = [buf.add(epb).to(b.add(m - qb)).raw_mut(), (b.add(m - qb), qb)];

        let [mut i, mut j] = [0, 0];
        while i != n && j != m {
            let [l, r] = [a.add(i), b.add(j)];
            let right = less(&*r, &*l);
            [i, j] = [i + !right as usize, j + right as usize];
            ptr::swap_nonoverlapping(if right { r } else { l }, buf.add(i + j - 1), 1);
        }

        // After that step, we are left with some number of A-elements and B-elements; we scroll the
        // buffer past all the A-elements, and we finish by merging up with the saved A-block
        buf = scroll_right(buf.add(i + j), n - i, epb - j);
        merge_up::<_, true>([buf_origin.crop(0..epb), buf.crop(epb..epb + m - j)], less);
    }

    Done
}

// Perform a block merge without a scrolling buffer.
//
// Cost: `O(n)` comparisons and `O(n)` moves.
unsafe fn in_place_block_merge<T, F: FnMut(&T, &T) -> bool>(
    keys: &mut Keys<T>, [a, b]: [&mut [T]; 2], less: &mut F,
) -> Sorted {
    // `tags` points to the start of the tags portion of our key collection
    // `na` and `nb` count the number of A and B blocks
    // `qa` and `qb` are the size of the undersized A and B blocks
    let tags = keys.inner.as_mut_ptr();
    let [(a, n), (_, m)] = [a, b].map(RawMut::raw_mut);
    let epb = (n + m) / keys.inner.len() + 1;
    let [na, nb, qa, qb] = [n / epb, m / epb, n % epb, m % epb];
    let s = a.add(qa);

    // We have to sort the first `na` keys in our key collection to use as tags
    keys.sort_first(na, less);
    (0..na).for_each(|i| ptr::swap(tags.add(i), s.add(i * epb + 1)));

    let mut prev = a.crop(0..qa);
    if (MergeContext {
        constants: (s, tags, na, nb, epb),
        on_drop: &mut |id, pid, _, less| {
            let next = prev.as_mut_ptr().add(prev.len()).crop(0..epb);
            if id == *pid {
                prev = next;
            } else {
                internal::merge_right(&mut prev, next, pid, less);
            }
        },
        init_min: &mut |dropped| dropped,
    }).merge_on(0..na + nb, less) == Block::A {
        // The rest of the elements are from A; merge the undersized B-block in
        crate::merge::merge_left([a.crop(0..n + m - qb), a.crop(n + m - qb..n + m)], less);
    }

    Done
}
