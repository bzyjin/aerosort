# aerosort

**aerosort** is an implementation of [block merge sort](https://en.wikipedia.org/wiki/Block_sort) with fairly simple algorithmic logic. It is comparison-based, stable, and in-place by default.

```
time (O)
    worst       average     best
    n log n     n log n     n
                            ^ only when the array contains one distinct value
space (O)
    minimum     maximum
    1           n
                ^ you can provide more, but it will only use up to n / 2 elements

characteristics
    + stable
    + comparison-based
    + deterministic
    + strict weak ordering
    - offline
    - quasi-adaptive (depends on cardinality)
```

> [!CAUTION]  
> aerosort has not yet been stabilized, made completely safe, or fully tested for correctness. I would certainly be thankful if someone wanted to help.

> [!TIP]  
> Providing `⌊n / 2⌋` elements or greater of external space will result in aerosort performing the equivalent of a top-down merge sort + insertion sort hybrid.

#### Agenda

```
- performance tests on various architectures
- prove/rigorously test correctness
- safety
- simplify logic even further (remove block selection "optimizations"?)
```

## Interface

The following interface is provided:

| Family       | Heap allocation (elements) |
|--------------|----------------------------|
| `sort`       | none                       |
| `sort_with`  | given (variable)           |

To sort using a comparator, use the `_by` interface and pass a comparison function e.g. `sort_by(&mut v, cmp)`. This allows you to sort descending and into other desired patterns.

To sort by key, use the `_by_key` interface and pass a mapping e.g. `sort_by_key(&mut v, f)`. This will sort ascending by key (lowest keys first).

## Algorithm

The overall implementation is based on [GrailSort](https://github.com/Mrrl/GrailSort) (Andrey Astrelin) and [WikiSort](https://github.com/BonzaiThePenguin/WikiSort) (Mike McFadden).

### Collecting keys

When given a buffer of size `m >= ⌊n / 2⌋`, aerosort does not collect any keys. Otherwise, it always tries to collect around `sqrt 2n` keys. Key collection is done at the beginning like in GrailSort. With large `n`, we can reduce comparisons by around 1% by collecting `2 sqrt n` keys, but it make the redistribution step slower, so for simplicity we ignore that strategy.

Once key collection is done, we partition our keys into two portions. We never need to sort the tags portion -- it remains sorted between every merge operation. The array should look like this:
```
| tags | internal buffer | ................. |
 <-------- keys --------> <---- to sort ---->
```

If 12 or fewer keys are collected, aerosort performs [Lazy Stable Sort](https://github.com/Mrrl/GrailSort/blob/master/GrailSort.h#L384) and terminates.

### Merging strategy

aerosort merges two runs in the following order of priority (i.e. it uses the first applicable strategy):
1. Copy one run and merge with the external buffer
2. Swap one run and merge with the internal buffer
3. Perform a scrolling block merge
4. Perform a rotation-based block merge

### Block merging

We tag every A-block (except the last for scrolling block merges) like in WikiSort &mdash; aerosort's scrolling block merge is a slightly modified version of GrailSort's in order to support different merging orders. 

#### Isolated merges

_We will use terminology from [this video](https://www.youtube.com/watch?v=InGeRuRk3f8&pp=ygUJZ3JhaWxzb3J0)_.

Suppose we want to merge two runs A and B. Instead of assuming the internal buffer is on the left, we perform the following operations before a scrolling block merge:
1. Swap the first full A-block with the internal buffer
2. Scroll the internal buffer to the left of the undersized A-block -- the internal buffer is now in position
3. Swap the saved A-block with the last A-block (as the buffer scrolls to the right, we want to handle the final A-block, not the first)

### Advantages

The most important advantages of our merging implementation is that we can have arbitrary merges -- the location and order of our merges does not matter. As long as we have collected either the desired number of keys or all unique values from an array, we can just "call `merge()`" on any two runs to merge them. This results in both a simpler sorting loop and a more general implementation.

#### Application in hybrid sorting algorithms

Because we can perform such merges, we can substitute our strategy for regular merges in adaptive/natural sorting algorithms like [Powersort](https://github.com/sebawild/powersort).

### Merge sort loop

In the interest of having `O(1)` space complexity, we emulate a top-down merge sort using iteration. I chose the following design over an ordinary bottom-up merge sort, (again) for simplicity and because it matches the merging order of top-down merge sort:

```
MergeDepth(i)
    return largest k such that 2^k is a factor of i

MergeSort(A[0 : n])
    MIN = 16    // small run length

    factor = 1 << Log2Ceil(n / MIN)     // # of small runs
    RunBound(i) = n * i / factor

    for i = 1 to factor:
        mid = RunBound(i - 1); right = RunBound(i)
        SortSmall(A[mid..right])

        for k = 0 to MergeDepth(i) - 1:
            left = RunBound(i - (2 << k)); mid = RunBound(i - (1 << k))
            Merge(A[left : mid], A[mid : right])
```

This is also possible only because of our generalized merging implementation.

## Attributions

Many thanks to:
- [Andrey Astrelin 🕊️](https://github.com/Mrrl) and [Mike McFadden](https://github.com/BonzaiThePenguin) for GrailSort and WikiSort 
- [Kuvina Saydaki](https://www.youtube.com/@Kuvina) for their [excellent explanation of GrailSort and WikiSort](https://www.youtube.com/watch?v=InGeRuRk3f8&pp=ygUJZ3JhaWxzb3J0)
- [Voultapher](https://github.com/Voultapher) for his work on [tiny-sort-rs](https://github.com/Voultapher/tiny-sort-rs) which I referenced when implementing aerosort
- [MusicTheorist](https://github.com/MusicTheorist) and their community for their [GrailSort visualizations](https://www.youtube.com/playlist?list=PL5w_-zMAJC8sF-bThVsDGthPcxJktuUNm)
